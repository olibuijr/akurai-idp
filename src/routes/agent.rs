use axum::{
    Form, Router,
    extract::Extension,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::config;
use crate::lib::html::{console_page, esc_html};
use crate::middleware::auth::AuthUser;

const MAX_PROMPT_CHARS: usize = 8_000;
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
    Html(console_page(
        "Agent Console",
        &agent_body(&user, &csrf, "", None),
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
        return Html(console_page(
            "Agent Console",
            &agent_body(&user, &csrf, prompt, Some(outcome)),
        ))
        .into_response();
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        let outcome = AgentOutcome::error("Prompt is too long for this console.");
        return Html(console_page(
            "Agent Console",
            &agent_body(&user, &csrf, prompt, Some(outcome)),
        ))
        .into_response();
    }

    let outcome = match query_agent(&user, prompt).await {
        Ok(outcome) => outcome,
        Err(error) => AgentOutcome::error(&error),
    };
    Html(console_page(
        "Agent Console",
        &agent_body(&user, &csrf, prompt, Some(outcome)),
    ))
    .into_response()
}

fn agent_body(user: &AuthUser, csrf: &str, prompt: &str, outcome: Option<AgentOutcome>) -> String {
    let cfg = config::get();
    let response_html = outcome.map(render_outcome).unwrap_or_else(|| {
        r#"<section class="agent-output agent-output-empty" aria-live="polite">
  <div class="agent-output-kicker">Ready</div>
  <pre>Awaiting input.</pre>
</section>"#
            .to_string()
    });

    format!(
        r#"<section class="agent-console" aria-label="AkurAI agent console">
  <div class="agent-rail">
    <div class="agent-chip agent-chip-live">Authenticated</div>
    <div class="agent-chip">{email}</div>
    <div class="agent-chip">{provider}</div>
    <div class="agent-chip">{model}</div>
  </div>

  <div class="agent-grid">
    <form class="agent-command" method="post" action="">
      <input type="hidden" name="_csrf" value="{csrf}">
      <label for="prompt">Command</label>
      <textarea id="prompt" name="prompt" rows="9" maxlength="{max_prompt}" required spellcheck="false" autocomplete="off" placeholder="Ask Rust Agent...">{prompt}</textarea>
      <div class="agent-command-footer">
        <a class="action-link" href="/account">Account</a>
        <button type="submit" class="btn btn-primary">Run</button>
      </div>
    </form>

    {response_html}
  </div>
</section>"#,
        csrf = esc_html(csrf),
        email = esc_html(&user.email),
        provider = esc_html(&cfg.agent_provider),
        model = esc_html(&cfg.agent_model),
        max_prompt = MAX_PROMPT_CHARS,
        prompt = esc_html(prompt),
    )
}

fn render_outcome(outcome: AgentOutcome) -> String {
    let class = if outcome.ok {
        "agent-output"
    } else {
        "agent-output agent-output-error"
    };
    let meta = if outcome.ok {
        format!(
            r#"<div class="agent-output-meta">
  <span>{provider}</span><span>{model}</span><span>{session}</span><span>job {job}</span>
</div>"#,
            provider = esc_html(&outcome.provider),
            model = esc_html(&outcome.model),
            session = esc_html(&outcome.session_id),
            job = outcome.job_id.unwrap_or_default(),
        )
    } else {
        String::new()
    };

    format!(
        r#"<section class="{class}" aria-live="polite">
  <div class="agent-output-kicker">{kicker}</div>
  {meta}
  <pre>{response}</pre>
</section>"#,
        class = class,
        kicker = if outcome.ok { "Response" } else { "Error" },
        meta = meta,
        response = esc_html(&outcome.response),
    )
}

fn forbidden_page(user: &AuthUser) -> Response {
    let body = format!(
        r#"<section class="agent-console">
  <div class="agent-output agent-output-error">
    <div class="agent-output-kicker">Access denied</div>
    <pre>{email} is not enabled for the AkurAI agent console.</pre>
  </div>
  <p class="page-footer"><a href="/account">Back to account</a></p>
</section>"#,
        email = esc_html(&user.email),
    );
    (
        StatusCode::FORBIDDEN,
        Html(console_page("Agent Console", &body)),
    )
        .into_response()
}

async fn query_agent(user: &AuthUser, prompt: &str) -> Result<AgentOutcome, String> {
    let cfg = config::get();
    let body = json!({
        "prompt": prompt,
        "provider": cfg.agent_provider,
        "model": cfg.agent_model,
        "scope_id": format!("idp:{}", user.tenant_id),
        "session_id": format!("idp:{}:agent", user.email),
        "user": user.email,
    });
    let response = post_json(&cfg.agent_gateway_url, &body.to_string()).await?;
    AgentOutcome::from_gateway_json(&response)
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
struct AgentOutcome {
    ok: bool,
    response: String,
    provider: String,
    model: String,
    session_id: String,
    job_id: Option<i64>,
}

impl AgentOutcome {
    fn error(message: &str) -> Self {
        Self {
            ok: false,
            response: message.to_string(),
            provider: String::new(),
            model: String::new(),
            session_id: String::new(),
            job_id: None,
        }
    }

    fn from_gateway_json(raw: &str) -> Result<Self, String> {
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
            session_id: value
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            job_id: value.get("job_id").and_then(Value::as_i64),
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
            r#"{"status":"ok","provider":"openai-codex","model":"gpt-5.4-mini","session_id":"s","job_id":7,"response":"hello"}"#,
        )
        .unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.response, "hello");
        assert_eq!(outcome.job_id, Some(7));
    }
}
