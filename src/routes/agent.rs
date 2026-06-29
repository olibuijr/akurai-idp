use async_stream::stream;
use axum::{
    Form, Json, Router,
    body::{Body, Bytes},
    extract::{Extension, Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::config;
use crate::lib::html::console_page_with_theme;
use crate::middleware::auth::AuthUser;
use crate::routes::agent_view::{
    AGENT_OS_STYLES, AgentPage, agent_body, agent_static_page_body, forbidden_body,
};

pub(crate) const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

pub fn router() -> Router {
    Router::new()
        .route("/agent", get(agent_page).post(agent_submit))
        .route("/agent/kanban", get(agent_kanban_page))
        .route("/agent/run", get(agent_run_page))
        .route("/agent/kanban/boards", get(kanban_boards))
        .route("/agent/kanban/board/{board}", get(kanban_board))
        .route("/agent/kanban/tasks", post(kanban_create_task))
        .route("/agent/kanban/tasks/{task_id}", get(kanban_task))
        .route(
            "/agent/kanban/tasks/{task_id}/status",
            post(kanban_task_status),
        )
        .route(
            "/agent/kanban/tasks/{task_id}/assign",
            post(kanban_task_assign),
        )
        .route(
            "/agent/kanban/tasks/{task_id}/comments",
            post(kanban_task_comment),
        )
        .route(
            "/agent/kanban/tasks/{task_id}/claim",
            post(kanban_task_claim),
        )
        .route(
            "/agent/kanban/tasks/{task_id}/heartbeat",
            post(kanban_task_heartbeat),
        )
        .route("/agent/kanban/reclaim", post(kanban_reclaim))
        .route("/agent/kanban/dispatch", post(kanban_dispatch))
        .route("/agent/stream", post(agent_stream))
}

#[derive(Debug, Deserialize)]
struct AgentForm {
    prompt: Option<String>,
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanBoardQuery {
    include_done: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanTaskInput {
    title: Option<String>,
    description: Option<String>,
    assignee: Option<String>,
    board: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanStatusInput {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanAssignInput {
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanCommentInput {
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanHeartbeatInput {
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KanbanDispatchInput {
    dry_run: Option<bool>,
    max_claims: Option<usize>,
}

async fn agent_page(headers: HeaderMap, Extension(user): Extension<AuthUser>) -> Response {
    if !agent_allowed(&user.email) {
        return forbidden_page(&headers, &user);
    }
    let csrf = csrf_cookie(&headers).unwrap_or_default();
    let theme = agent_theme(&headers);
    Html(console_page_with_theme(
        "Agent Console",
        &agent_body(&user, &csrf, "", None),
        AGENT_OS_STYLES,
        Some(&theme),
    ))
    .into_response()
}

async fn agent_kanban_page(headers: HeaderMap, Extension(user): Extension<AuthUser>) -> Response {
    agent_workspace_page(headers, user, AgentPage::Kanban).await
}

async fn agent_run_page(headers: HeaderMap, Extension(user): Extension<AuthUser>) -> Response {
    agent_workspace_page(headers, user, AgentPage::Run).await
}

async fn agent_workspace_page(headers: HeaderMap, user: AuthUser, page: AgentPage) -> Response {
    if !agent_allowed(&user.email) {
        return forbidden_page(&headers, &user);
    }
    let csrf = csrf_cookie(&headers).unwrap_or_default();
    let theme = agent_theme(&headers);
    Html(console_page_with_theme(
        page.title(),
        &agent_static_page_body(&user, &csrf, page),
        AGENT_OS_STYLES,
        Some(&theme),
    ))
    .into_response()
}

async fn agent_submit(
    headers: HeaderMap,
    Extension(user): Extension<AuthUser>,
    Form(form): Form<AgentForm>,
) -> Response {
    if !agent_allowed(&user.email) {
        return forbidden_page(&headers, &user);
    }

    let csrf = csrf_cookie(&headers).unwrap_or_default();
    let theme = agent_theme(&headers);
    let prompt = form.prompt.unwrap_or_default();
    let prompt = prompt.trim();
    if let Some(message) = validate_prompt(prompt) {
        let outcome = AgentOutcome::error(message);
        return Html(console_page_with_theme(
            "Agent Console",
            &agent_body(&user, &csrf, prompt, Some(&outcome)),
            AGENT_OS_STYLES,
            Some(&theme),
        ))
        .into_response();
    }

    let outcome = match query_agent(&user, prompt).await {
        Ok(outcome) => outcome,
        Err(error) => AgentOutcome::error(&error),
    };
    Html(console_page_with_theme(
        "Agent Console",
        &agent_body(&user, &csrf, prompt, Some(&outcome)),
        AGENT_OS_STYLES,
        Some(&theme),
    ))
    .into_response()
}

async fn agent_stream(
    Extension(user): Extension<AuthUser>,
    Form(form): Form<AgentForm>,
) -> Response {
    if !agent_allowed(&user.email) {
        return (StatusCode::FORBIDDEN, "Agent access denied.").into_response();
    }

    let prompt = form.prompt.unwrap_or_default().trim().to_string();
    if let Some(message) = validate_prompt(&prompt) {
        return sse_text_response(stream_event_text(
            "error",
            json!({
                "ok": false,
                "response": message,
            }),
        ));
    }

    let body_stream = stream! {
        yield Ok::<Bytes, Infallible>(Bytes::from(stream_event_text(
            "start",
            json!({
                "ok": true,
                "status": "started",
            }),
        )));

        let outcome = match query_agent(&user, &prompt).await {
            Ok(outcome) => outcome,
            Err(error) => AgentOutcome::error(&error),
        };

        yield Ok::<Bytes, Infallible>(Bytes::from(stream_event_text(
            "final",
            outcome_stream_payload(&outcome),
        )));
    };

    sse_body_response(Body::from_stream(body_stream))
}

async fn kanban_boards(Extension(user): Extension<AuthUser>) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_get("/kanban/boards").await
}

async fn kanban_board(
    Extension(user): Extension<AuthUser>,
    Path(board): Path<String>,
    Query(query): Query<KanbanBoardQuery>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    let include_done = query
        .include_done
        .as_deref()
        .map(|value| !matches!(value, "" | "0" | "false"))
        .unwrap_or(false);
    let path = format!(
        "/kanban/board/{}{}",
        encode_path_segment(&board),
        if include_done { "?include_done=1" } else { "" }
    );
    kanban_gateway_get(&path).await
}

async fn kanban_task(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_get(&format!("/kanban/tasks/{}", encode_path_segment(&task_id))).await
}

async fn kanban_create_task(
    Extension(user): Extension<AuthUser>,
    Json(input): Json<KanbanTaskInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        "/kanban/tasks",
        json!({
            "title": input.title.unwrap_or_default(),
            "description": input.description.unwrap_or_default(),
            "assignee": input.assignee.unwrap_or_default(),
            "board": input.board.unwrap_or_else(|| "default".to_string()),
        }),
    )
    .await
}

async fn kanban_task_status(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
    Json(input): Json<KanbanStatusInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        &format!("/kanban/tasks/{}/status", encode_path_segment(&task_id)),
        json!({ "status": input.status.unwrap_or_else(|| "todo".to_string()) }),
    )
    .await
}

async fn kanban_task_assign(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
    Json(input): Json<KanbanAssignInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        &format!("/kanban/tasks/{}/assign", encode_path_segment(&task_id)),
        json!({ "assignee": input.assignee.unwrap_or_default() }),
    )
    .await
}

async fn kanban_task_comment(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
    Json(input): Json<KanbanCommentInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        &format!("/kanban/tasks/{}/comments", encode_path_segment(&task_id)),
        json!({ "body": input.body.unwrap_or_default(), "author": user.email }),
    )
    .await
}

async fn kanban_task_claim(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        &format!("/kanban/tasks/{}/claim", encode_path_segment(&task_id)),
        json!({}),
    )
    .await
}

async fn kanban_task_heartbeat(
    Extension(user): Extension<AuthUser>,
    Path(task_id): Path<String>,
    Json(input): Json<KanbanHeartbeatInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        &format!("/kanban/tasks/{}/heartbeat", encode_path_segment(&task_id)),
        json!({ "note": input.note.unwrap_or_default() }),
    )
    .await
}

async fn kanban_reclaim(Extension(user): Extension<AuthUser>) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post("/kanban/reclaim", json!({})).await
}

async fn kanban_dispatch(
    Extension(user): Extension<AuthUser>,
    Json(input): Json<KanbanDispatchInput>,
) -> Response {
    if !agent_allowed(&user.email) {
        return kanban_forbidden();
    }
    kanban_gateway_post(
        "/kanban/dispatch",
        json!({
            "dry_run": input.dry_run.unwrap_or(true),
            "max_claims": input.max_claims,
        }),
    )
    .await
}

fn forbidden_page(headers: &HeaderMap, user: &AuthUser) -> Response {
    let theme = agent_theme(headers);
    (
        StatusCode::FORBIDDEN,
        Html(console_page_with_theme(
            "Agent Console",
            &forbidden_body(user),
            AGENT_OS_STYLES,
            Some(&theme),
        )),
    )
        .into_response()
}

fn validate_prompt(prompt: &str) -> Option<&'static str> {
    if prompt.is_empty() {
        return Some("Prompt is empty.");
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Some("Prompt is too long for this console.");
    }
    None
}

async fn query_agent(user: &AuthUser, prompt: &str) -> Result<AgentOutcome, String> {
    let cfg = config::get();
    let scope_id = format!("idp:{}", user.tenant_id);
    let session_id = format!("idp:{}:agent", user.email);
    let body = json!({
        "prompt": prompt,
        "provider": cfg.agent_provider,
        "model": cfg.agent_model,
        "scope_id": scope_id,
        "session_id": session_id,
        "user": user.email,
    });
    let started = Instant::now();
    let response = post_json(&cfg.agent_gateway_url, &body.to_string()).await?;
    AgentOutcome::from_gateway_json(&response, started.elapsed().as_millis() as u64)
}

fn outcome_stream_payload(outcome: &AgentOutcome) -> Value {
    json!({
        "ok": outcome.ok,
        "response": &outcome.response,
        "provider": &outcome.provider,
        "model": &outcome.model,
        "scope_id": &outcome.scope_id,
        "session_id": &outcome.session_id,
        "job_id": outcome.job_id,
        "latency_ms": outcome.latency_ms,
        "tool_call_id": outcome.tool_call_id(),
    })
}

fn stream_event_text(event: &str, data: Value) -> String {
    let data = serde_json::to_string(&data).unwrap_or_else(|_| {
        r#"{"ok":false,"response":"Agent stream serialization failed."}"#.to_string()
    });
    format!("event: {event}\ndata: {data}\n\n")
}

fn sse_text_response(event: String) -> Response {
    sse_body_response(Body::from(event))
}

fn sse_body_response(body: Body) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .header("x-accel-buffering", "no")
        .body(body)
        .unwrap()
}

async fn post_json(url: &str, body: &str) -> Result<String, String> {
    request_json("POST", url, Some(body)).await
}

async fn kanban_gateway_get(path: &str) -> Response {
    match request_json("GET", &gateway_api_url(path), None).await {
        Ok(body) => kanban_json_response(StatusCode::OK, &body),
        Err(error) => kanban_error(StatusCode::BAD_GATEWAY, &error),
    }
}

async fn kanban_gateway_post(path: &str, body: Value) -> Response {
    match request_json("POST", &gateway_api_url(path), Some(&body.to_string())).await {
        Ok(body) => kanban_json_response(StatusCode::OK, &body),
        Err(error) => kanban_error(StatusCode::BAD_GATEWAY, &error),
    }
}

fn gateway_api_url(path: &str) -> String {
    format!("{}{}", config::get().agent_gateway_base_url, path)
}

fn kanban_json_response(status: StatusCode, body: &str) -> Response {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => (status, Json(value)).into_response(),
        Err(_) => kanban_error(
            StatusCode::BAD_GATEWAY,
            "Agent gateway returned invalid kanban JSON.",
        ),
    }
}

fn kanban_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "status": "error", "error": message }))).into_response()
}

fn kanban_forbidden() -> Response {
    kanban_error(StatusCode::FORBIDDEN, "Agent access denied.")
}

async fn request_json(method: &str, url: &str, body: Option<&str>) -> Result<String, String> {
    let target = HttpTarget::parse(url)?;
    let mut stream = TcpStream::connect((&target.host[..], target.port))
        .await
        .map_err(|error| format!("Agent gateway connect failed: {error}"))?;
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {len}\r\n\r\n{body}",
        method = method,
        path = target.path,
        host = target.host_header(),
        len = body.len(),
        body = body,
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| format!("Agent gateway write failed: {error}"))?;

    let mut raw = Vec::new();
    stream
        .take(MAX_RESPONSE_BYTES as u64)
        .read_to_end(&mut raw)
        .await
        .map_err(|error| format!("Agent gateway read failed: {error}"))?;
    parse_http_response(&raw)
}

fn parse_http_response(raw: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "Agent gateway returned an invalid HTTP response.".to_string())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("Agent gateway returned HTTP {status}: {body}"));
    }
    Ok(body.to_string())
}

fn agent_allowed(email: &str) -> bool {
    let email = email.trim().to_ascii_lowercase();
    config::get()
        .agent_allowed_emails
        .iter()
        .any(|allowed| allowed == "*" || allowed == &email)
}

fn csrf_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, "_csrf")
}

fn agent_theme(headers: &HeaderMap) -> String {
    suite_theme_cookie(headers).unwrap_or_else(|| "claude-code".to_string())
}

fn suite_theme_cookie(headers: &HeaderMap) -> Option<String> {
    let theme = cookie_value(headers, "akurai-theme")?;
    match theme.as_str() {
        "akurai" | "akurai-light" | "claude-code" | "claude-code-light" | "nord" | "nord-light"
        | "catppuccin-mocha" | "catppuccin-latte" | "solarized-dark" | "solarized-light"
        | "gruvbox-dark" | "gruvbox-light" | "tokyo-night" | "tokyo-night-light" | "rose-pine"
        | "rose-pine-dawn" | "dracula" => Some(theme),
        _ => None,
    }
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

fn encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(crate) struct AgentOutcome {
    pub ok: bool,
    pub response: String,
    pub provider: String,
    pub model: String,
    pub scope_id: String,
    pub session_id: String,
    pub job_id: Option<i64>,
    pub latency_ms: Option<u64>,
}

impl AgentOutcome {
    fn error(message: &str) -> Self {
        Self {
            ok: false,
            response: message.to_string(),
            provider: String::new(),
            model: String::new(),
            scope_id: String::new(),
            session_id: String::new(),
            job_id: None,
            latency_ms: None,
        }
    }

    pub(crate) fn tool_call_id(&self) -> String {
        self.job_id
            .map(|job_id| format!("gateway-query-{job_id}"))
            .unwrap_or_else(|| "gateway-query-local".to_string())
    }

    pub(crate) fn was_gateway_attempted(&self) -> bool {
        self.latency_ms.is_some()
    }

    fn from_gateway_json(raw: &str, latency_ms: u64) -> Result<Self, String> {
        let value: Value = serde_json::from_str(raw)
            .map_err(|error| format!("Agent gateway JSON parse failed: {error}"))?;
        if value.get("status").and_then(Value::as_str) != Some("ok") {
            let error = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Agent gateway returned an error.");
            return Ok(Self::error(error));
        }
        Ok(Self {
            ok: true,
            response: value
                .get("response")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            provider: value
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            scope_id: value
                .get("scope_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            session_id: value
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            job_id: value.get("job_id").and_then(Value::as_i64),
            latency_ms: Some(latency_ms),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpTarget {
    host: String,
    port: u16,
    path: String,
}

impl HttpTarget {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "Agent gateway URL must use plain http://".to_string())?;
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (rest, "/".to_string()),
        };
        if authority.is_empty() {
            return Err("Agent gateway URL is missing a host.".to_string());
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, raw_port)) => {
                let port = raw_port
                    .parse::<u16>()
                    .map_err(|_| "Agent gateway URL has an invalid port.".to_string())?;
                (host.to_string(), port)
            }
            None => (authority.to_string(), 80),
        };
        if host.is_empty() {
            return Err("Agent gateway URL is missing a host.".to_string());
        }
        Ok(Self { host, port, path })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_gateway_url() {
        let target = HttpTarget::parse("http://127.0.0.1:8644/query").unwrap();
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 8644);
        assert_eq!(target.path, "/query");
        assert_eq!(target.host_header(), "127.0.0.1:8644");
    }

    #[test]
    fn parses_gateway_json_response() {
        let outcome = AgentOutcome::from_gateway_json(
            r#"{"status":"ok","provider":"openai-codex","model":"gpt-5.4-mini","scope_id":"scope","session_id":"s","job_id":7,"response":"hello"}"#,
            123,
        )
        .unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.response, "hello");
        assert_eq!(outcome.scope_id, "scope");
        assert_eq!(outcome.job_id, Some(7));
        assert_eq!(outcome.latency_ms, Some(123));
    }

    #[test]
    fn validates_stream_prompt() {
        assert_eq!(validate_prompt(""), Some("Prompt is empty."));
        assert_eq!(validate_prompt("hello"), None);
    }

    #[test]
    fn formats_stream_event() {
        let event = stream_event_text("final", json!({"ok": true, "response": "hello"}));
        assert!(event.starts_with("event: final\n"));
        assert!(event.contains(r#""response":"hello""#));
        assert!(event.ends_with("\n\n"));
    }
}
