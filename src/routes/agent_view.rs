use crate::config;
use crate::lib::html::esc_html;
use crate::middleware::auth::AuthUser;
use crate::routes::agent::{AgentOutcome, MAX_PROMPT_CHARS};

pub const AGENT_OS_STYLES: &str = include_str!("agent_os.css");

pub(crate) fn agent_body(
    user: &AuthUser,
    csrf: &str,
    prompt: &str,
    outcome: Option<&AgentOutcome>,
) -> String {
    let cfg = config::get();
    let timeline = match outcome {
        Some(outcome) => render_timeline(prompt, outcome),
        None => render_ready_timeline(),
    };
    let session_id = agent_session_id(user);
    let scope_id = agent_scope_id(user);

    format!(
        r#"<section class="agent-os chat-root" aria-label="AkurAI-RustAgent">
  <aside class="agent-sidebar" aria-label="Workspace">
    <div class="agent-product">
      <div class="agent-avatar" aria-hidden="true">{initials}</div>
      <div class="agent-brand">RustAgent<small>Hermes runtime</small></div>
    </div>
    <a class="agent-new-task" href="/agent">New chat</a>
    <nav class="agent-modebar" aria-label="Modes">
      <span class="agent-mode agent-mode-active" data-icon="C">Chat</span>
      <span class="agent-mode" data-icon="T">Tasks</span>
      <span class="agent-mode" data-icon="P">Projects</span>
    </nav>
    <div class="agent-section">
      <h2>Workspace</h2>
      <nav class="agent-nav" aria-label="Workspace state">
        <span class="active">Current run <b class="agent-count">live</b></span>
        <span>AGY <b class="agent-count">context</b></span>
        <span>Notes <b class="agent-count">local</b></span>
        <span>Passvault <b class="agent-count">sealed</b></span>
      </nav>
    </div>
    <div class="agent-section">
      <h2>Queues</h2>
      <nav class="agent-nav" aria-label="Queues">
        <span>Cron <b class="agent-count">ready</b></span>
        <span>Kanban <b class="agent-count">ready</b></span>
        <span>Curator <b class="agent-count">ready</b></span>
      </nav>
    </div>
    <div class="agent-section agent-account">
      <h2>Account</h2>
      <a href="/account">AkurAI ID</a>
    </div>
  </aside>

  <section class="agent-main" aria-label="Conversation">
    <header class="agent-head">
      <div>
        <h1 class="agent-title">olibuijr</h1>
        <p class="agent-subtitle">AkurAI-RustAgent</p>
      </div>
      <div class="agent-meta" aria-label="Runtime">
        <span>{provider}</span>
        <span>{model}</span>
      </div>
    </header>

    <div class="agent-timeline chat-thread" aria-live="polite">
      {timeline}
    </div>

    <form class="agent-composer chat-composer" method="post" action="" aria-label="Message RustAgent">
      <input type="hidden" name="_csrf" value="{csrf}">
      <textarea id="prompt" name="prompt" maxlength="{max_prompt}" required spellcheck="false" autocomplete="off" aria-label="Message RustAgent" placeholder="Message RustAgent...">{prompt}</textarea>
      <div class="agent-composer-footer">
        <span class="agent-state">{scope_id}</span>
        <button type="submit" class="btn btn-primary">Run</button>
      </div>
    </form>
  </section>

  <aside class="agent-context" aria-label="Run details">
    {context}
  </aside>
  <div class="agent-protocol" hidden data-session="{session_id}">
    analysis commentary final tool_call tool_result approval question edit artifact system error
    clarify.request approval.request sudo.request secret.request terminal.read.request
  </div>
</section>"#,
        initials = esc_html(&agent_initials(&user.email)),
        provider = esc_html(&cfg.agent_provider),
        model = esc_html(&cfg.agent_model),
        csrf = esc_html(csrf),
        max_prompt = MAX_PROMPT_CHARS,
        prompt = esc_html(prompt),
        session_id = esc_html(&session_id),
        scope_id = esc_html(&scope_id),
        timeline = timeline,
        context = render_context(user),
    )
}

pub(crate) fn forbidden_body(user: &AuthUser) -> String {
    format!(
        r#"<section class="agent-os chat-root" aria-label="AkurAI-RustAgent">
  <section class="agent-main" style="grid-column: 1 / -1;">
    <div class="agent-timeline chat-thread">
      <article class="agent-event agent-event-error chat-message" data-channel="error">
        <div class="agent-event-head">
          <span class="agent-channel">system</span>
          <span class="agent-time">access denied</span>
        </div>
        <pre class="chat-message-content">{email} is not enabled for RustAgent.</pre>
      </article>
    </div>
  </section>
</section>"#,
        email = esc_html(&user.email),
    )
}

fn render_ready_timeline() -> String {
    r#"<section class="agent-empty chat-message" data-channel="system" data-kind="ready">
  <h2>What should we work on?</h2>
  <p>RustAgent is connected to the stable gateway.</p>
  <div class="agent-suggestions" aria-label="Suggestions">
    <span>Inspect rust-agent</span>
    <span>Plan a deploy</span>
    <span>Open AGY context</span>
  </div>
</section>"#
        .to_string()
}

fn render_timeline(prompt: &str, outcome: &AgentOutcome) -> String {
    let status_class = if outcome.ok {
        "agent-event"
    } else {
        "agent-event agent-event-error"
    };
    let status = if outcome.ok { "complete" } else { "error" };
    let run_details = if outcome.was_gateway_attempted() {
        format!(
            r#"<details class="agent-run-details chat-toolcall" data-channel="tool_call" data-kind="gateway.query" data-tool-call-id="{tool_call_id}">
  <summary>Ran with {model} in {latency} ms</summary>
  <dl class="agent-tool-grid">{tool_meta}</dl>
</details>"#,
            tool_call_id = esc_html(&outcome.tool_call_id()),
            model = esc_html(&outcome.model),
            latency = outcome.latency_ms.unwrap_or_default(),
            tool_meta = render_tool_meta(outcome),
        )
    } else {
        String::new()
    };

    format!(
        r#"<article class="agent-event agent-event-user chat-message chat-message-user" data-channel="user" data-kind="message">
  <div class="agent-event-head">
    <span class="agent-channel">you</span>
    <span class="agent-time">submitted</span>
  </div>
  <pre class="chat-message-content">{prompt}</pre>
</article>
{run_details}
<article class="{status_class} agent-event-assistant chat-message chat-message-assistant" data-channel="final" data-kind="{status}">
  <div class="agent-event-head">
    <span class="agent-channel">{channel}</span>
    <span class="agent-time">{latency} ms</span>
  </div>
  <pre class="chat-message-content">{response}</pre>
</article>"#,
        prompt = esc_html(prompt),
        run_details = run_details,
        status_class = status_class,
        status = status,
        channel = if outcome.ok {
            "rustagent"
        } else {
            "gateway error"
        },
        latency = outcome.latency_ms.unwrap_or_default(),
        response = esc_html(&outcome.response),
    )
}

fn render_tool_meta(outcome: &AgentOutcome) -> String {
    let rows = [
        ("provider", outcome.provider.as_str()),
        ("model", outcome.model.as_str()),
        ("scope", outcome.scope_id.as_str()),
        ("session", outcome.session_id.as_str()),
    ];
    rows.into_iter()
        .map(|(label, value)| {
            format!(
                r#"<div class="agent-kv"><dt>{label}</dt><dd>{value}</dd></div>"#,
                label = esc_html(label),
                value = esc_html(value),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_context(user: &AuthUser) -> String {
    let session_id = agent_session_id(user);
    let scope_id = agent_scope_id(user);

    format!(
        r#"<div class="agent-context-grid">
  <section class="agent-context-card">
    <h3>Run</h3>
    <p>{email}</p>
    <p class="mono">{scope}</p>
    <p class="mono">{session}</p>
  </section>
  <section class="agent-context-card">
    <h3>Workspace</h3>
    <p>AGY, notes, passvault, cron, kanban, curator.</p>
  </section>
</div>"#,
        email = esc_html(&user.email),
        scope = esc_html(&scope_id),
        session = esc_html(&session_id),
    )
}

fn agent_scope_id(user: &AuthUser) -> String {
    format!("idp:{}", user.tenant_id)
}

fn agent_session_id(user: &AuthUser) -> String {
    format!("idp:{}:agent", user.email)
}

fn agent_initials(email: &str) -> String {
    email.chars().next().unwrap_or('A').to_uppercase().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> AuthUser {
        AuthUser {
            id: "u1".to_string(),
            tenant_id: "tenant1".to_string(),
            email: "olibuijr@olibuijr.com".to_string(),
            email_verified: true,
        }
    }

    #[test]
    fn body_contains_agent_os_surfaces() {
        let html = agent_body(&user(), "csrf", "", None);
        assert!(html.contains("AkurAI-RustAgent"));
        assert!(html.contains("Passvault"));
        assert!(html.contains("Kanban"));
        assert!(html.contains("approval.request"));
        assert!(html.contains("tool_call"));
    }

    #[test]
    fn body_escapes_prompt() {
        let html = agent_body(&user(), "csrf", "<script>", None);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
