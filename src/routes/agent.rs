use axum::{
    Form, Router,
    extract::Extension,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::config;
use crate::lib::html::console_page_with_styles;
use crate::middleware::auth::AuthUser;
use crate::routes::agent_view::{AGENT_OS_STYLES, agent_body, forbidden_body};

pub(crate) const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

pub fn router() -> Router {
    Router::new().route("/agent", get(agent_page).post(agent_submit))
}

#[derive(Debug, Deserialize)]
struct AgentForm {
    prompt: Option<String>,
    _csrf: Option<String>,
}

async fn agent_page(headers: HeaderMap, Extension(user): Extension<AuthUser>) -> Response {
    if !agent_allowed(&user.email) {
        return forbidden_page(&user);
    }
    let csrf = csrf_cookie(&headers).unwrap_or_default();
    Html(console_page_with_styles(
        "Agent Console",
        &agent_body(&user, &csrf, "", None),
        AGENT_OS_STYLES,
    ))
    .into_response()
}

async fn agent_submit(
    headers: HeaderMap,
    Extension(user): Extension<AuthUser>,
    Form(form): Form<AgentForm>,
) -> Response {
    if !agent_allowed(&user.email) {
        return forbidden_page(&user);
    }

    let csrf = csrf_cookie(&headers).unwrap_or_default();
    let prompt = form.prompt.unwrap_or_default();
    let prompt = prompt.trim();
    if prompt.is_empty() {
        let outcome = AgentOutcome::error("Prompt is empty.");
        return Html(console_page_with_styles(
            "Agent Console",
            &agent_body(&user, &csrf, prompt, Some(&outcome)),
            AGENT_OS_STYLES,
        ))
        .into_response();
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        let outcome = AgentOutcome::error("Prompt is too long for this console.");
        return Html(console_page_with_styles(
            "Agent Console",
            &agent_body(&user, &csrf, prompt, Some(&outcome)),
            AGENT_OS_STYLES,
        ))
        .into_response();
    }

    let outcome = match query_agent(&user, prompt).await {
        Ok(outcome) => outcome,
        Err(error) => AgentOutcome::error(&error),
    };
    Html(console_page_with_styles(
        "Agent Console",
        &agent_body(&user, &csrf, prompt, Some(&outcome)),
        AGENT_OS_STYLES,
    ))
    .into_response()
}

fn forbidden_page(user: &AuthUser) -> Response {
    (
        StatusCode::FORBIDDEN,
        Html(console_page_with_styles(
            "Agent Console",
            &forbidden_body(user),
            AGENT_OS_STYLES,
        )),
    )
        .into_response()
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

async fn post_json(url: &str, body: &str) -> Result<String, String> {
    let target = HttpTarget::parse(url)?;
    let mut stream = TcpStream::connect((&target.host[..], target.port))
        .await
        .map_err(|error| format!("Agent gateway connect failed: {error}"))?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {len}\r\n\r\n{body}",
        path = target.path,
        host = target.host_header(),
        len = body.as_bytes().len(),
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
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("_csrf=") {
            return Some(value.to_string());
        }
    }
    None
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
}
