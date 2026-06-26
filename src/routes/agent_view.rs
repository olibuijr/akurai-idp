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
        r#"<section class="agent-os chat-root" aria-label="AkurAI-RustAgent workspace">
  <aside class="agent-sidebar" aria-label="Agent workspace navigation">
    <div class="agent-product">
      <div class="agent-avatar" aria-hidden="true">{initials}</div>
      <div class="agent-brand">AkurAI-RustAgent<small>agent.olibuijr.com</small></div>
    </div>
    <a class="agent-new-task" href="/agent">New task</a>
    <nav class="agent-modebar" aria-label="Agent modes">
      <span class="agent-mode agent-mode-active" data-icon="C">Chat</span>
      <span class="agent-mode" data-icon="K">Code</span>
      <span class="agent-mode" data-icon="W">Co-work</span>
      <span class="agent-mode" data-icon="A">Artifacts</span>
    </nav>
    <div class="agent-section">
      <h2>Workspace</h2>
      <nav class="agent-nav" aria-label="Workspace">
        <span class="active">This run <b class="agent-count">live</b></span>
        <span>AGY memory <b class="agent-count">context</b></span>
        <span>Notes <b class="agent-count">source</b></span>
        <span>passvault <b class="agent-count">sealed</b></span>
      </nav>
    </div>
    <div class="agent-section">
      <h2>Automation</h2>
      <nav class="agent-nav" aria-label="Automation">
        <span>Cron <b class="agent-count">ready</b></span>
        <span>Kanban <b class="agent-count">ready</b></span>
        <span>Curator <b class="agent-count">ready</b></span>
      </nav>
    </div>
    <p class="agent-footnote">Framework v0.8 theme tokens. Stable Rust gateway. Structured receipts reserved.</p>
    <div class="agent-section">
      <h2>Account</h2>
      <nav class="agent-nav" aria-label="Account links">
        <a href="/account">AkurAI ID <b class="agent-count">open</b></a>
      </nav>
    </div>
  </aside>

  <section class="agent-main" aria-label="Agent run">
    <header class="agent-head">
      <div class="agent-head-top">
        <div>
          <h1 class="agent-title">Welcome back, olibuijr</h1>
          <p class="agent-subtitle">Tenant agent OS for runs, tool calls, approvals, artifacts, AGY memory, notes, passvault, cron, kanban, and curator work.</p>
        </div>
        <div class="agent-meta" aria-label="Runtime metadata">
          <span class="agent-pill agent-pill-live">gateway stable</span>
          <span class="agent-pill">{provider}</span>
          <span class="agent-pill">{model}</span>
        </div>
      </div>
      <div class="agent-tabs" aria-label="Channels">
        <span class="agent-tab agent-tab-active">commentary</span>
        <span class="agent-tab">tool_call</span>
        <span class="agent-tab">approval</span>
        <span class="agent-tab">artifact</span>
        <span class="agent-tab">final</span>
      </div>
    </header>

    <div class="agent-timeline chat-thread" aria-live="polite">
      {timeline}
    </div>

    <form class="agent-composer chat-composer" method="post" action="" aria-label="Submit command to RustAgent">
      <input type="hidden" name="_csrf" value="{csrf}">
      <label for="prompt">Message AkurAI-RustAgent</label>
      <textarea id="prompt" name="prompt" maxlength="{max_prompt}" required spellcheck="false" autocomplete="off" placeholder="Ask RustAgent to inspect, plan, edit, deploy, or operate your workspace...">{prompt}</textarea>
      <div class="agent-composer-footer">
        <p class="agent-hint">Session <span class="mono">{session_id}</span></p>
        <div class="agent-actions">
          <span class="agent-state">{scope_id}</span>
          <button type="submit" class="btn btn-primary">Run</button>
        </div>
      </div>
    </form>
  </section>

  <aside class="agent-context" aria-label="Agent context">
    {context}
  </aside>
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
        r#"<section class="agent-os chat-root" aria-label="AkurAI-RustAgent workspace">
  <section class="agent-main" style="grid-column: 1 / -1;">
    <div class="agent-timeline chat-thread">
      <article class="agent-event agent-event-error chat-message" data-channel="error">
        <div class="agent-event-head">
          <span class="agent-channel">system.error</span>
          <span class="agent-time">access denied</span>
        </div>
        <pre class="chat-message-content">{email} is not enabled for the AkurAI-RustAgent workspace.</pre>
      </article>
    </div>
  </section>
</section>"#,
        email = esc_html(&user.email),
    )
}

fn render_ready_timeline() -> String {
    r#"<article class="agent-event agent-event-assistant chat-message chat-message-assistant" data-channel="system" data-kind="ready">
  <div class="agent-event-head">
    <span class="agent-channel">system</span>
    <span class="agent-time">awaiting input</span>
  </div>
  <pre class="chat-message-content">AkurAI-RustAgent is ready in this tenant workspace. The stable gateway can inspect, plan, and answer now; AGY memory, notes, passvault, cron, kanban, and curator stay pinned as first-class panes.</pre>
</article>
<article class="agent-event agent-event-tool chat-toolcall toolui-approval" data-channel="approval" data-kind="empty">
  <div class="agent-event-head">
    <span class="agent-channel">co-work queue</span>
    <span class="agent-time">no pending asks</span>
  </div>
  <div class="agent-request">
    <span>clarify.request</span>
    <span>approval.request</span>
    <span>sudo.request</span>
    <span>secret.request</span>
    <span>terminal.read.request</span>
  </div>
</article>"#
        .to_string()
}

fn render_timeline(prompt: &str, outcome: &AgentOutcome) -> String {
    let status_class = if outcome.ok {
        "agent-event"
    } else {
        "agent-event agent-event-error"
    };
    let status = if outcome.ok { "complete" } else { "error" };
    let tool_event = if outcome.was_gateway_attempted() {
        format!(
            r#"<article class="agent-event agent-event-tool chat-toolcall" data-channel="tool_call" data-kind="gateway.query" data-tool-call-id="{tool_call_id}">
  <div class="agent-event-head">
    <span class="agent-channel">gateway query</span>
    <span class="agent-time">{status}</span>
  </div>
  <dl class="agent-tool-grid">
    {tool_meta}
  </dl>
</article>"#,
            tool_call_id = esc_html(&outcome.tool_call_id()),
            status = status,
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
{tool_event}
<article class="{status_class} agent-event-assistant chat-message chat-message-assistant" data-channel="final" data-kind="{status}">
  <div class="agent-event-head">
    <span class="agent-channel">{channel}</span>
    <span class="agent-time">{latency} ms</span>
  </div>
  <pre class="chat-message-content">{response}</pre>
</article>"#,
        prompt = esc_html(prompt),
        tool_event = tool_event,
        status_class = status_class,
        channel = if outcome.ok {
            "AkurAI-RustAgent"
        } else {
            "gateway error"
        },
        latency = outcome.latency_ms.unwrap_or_default(),
        response = esc_html(&outcome.response),
    )
}

fn render_tool_meta(outcome: &AgentOutcome) -> String {
    let mut rows = vec![
        ("provider", outcome.provider.as_str()),
        ("model", outcome.model.as_str()),
        ("scope", outcome.scope_id.as_str()),
        ("session", outcome.session_id.as_str()),
    ];
    let job_id = outcome.job_id.map(|id| id.to_string()).unwrap_or_default();
    if !job_id.is_empty() {
        rows.push(("job", job_id.as_str()));
    }

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
  <section class="agent-context-card card">
    <h3>Run progress</h3>
    <p>Stable HTTP gateway with reserved lanes for streaming events and receipts.</p>
    <div class="agent-progress tc-progress-tracker">
      <div class="agent-progress-row"><span class="agent-progress-dot">1</span><span>Tenant session scoped</span></div>
      <div class="agent-progress-row"><span class="agent-progress-dot">2</span><span>Gateway query ready</span></div>
      <div class="agent-progress-row"><span class="agent-progress-dot">3</span><span>Receipts mapped</span></div>
    </div>
  </section>
  <section class="agent-context-card card">
    <h3>Artifacts</h3>
    <p>Diffs, previews, files, tables, and generated outputs stay out of plain chat text.</p>
    <div class="tc-code-diff"><b>+</b> code_diff.preview<br><b>+</b> terminal.output<br><b>+</b> curator.report</div>
  </section>
  <section class="agent-context-card card">
    <h3>Context</h3>
    <p>{email}</p>
    <ul>
      <li class="mono">{tenant}</li>
      <li class="mono">{scope}</li>
      <li class="mono">{session}</li>
    </ul>
  </section>
  <section class="agent-context-card card">
    <h3>Persistent workspace</h3>
    <p>AGY memory, notes, and passvault stay RustAgent-owned and tenant scoped.</p>
    <ul>
      <li>AGY memory context</li>
      <li>Durable notes</li>
      <li>Sealed passvault</li>
    </ul>
  </section>
  <section class="agent-context-card card toolui-approval">
    <h3>Pending confirmations</h3>
    <p>No active asks. Future receipt cards keep Hermes response methods intact.</p>
    <div class="agent-request">
      <span>approval.respond</span>
      <span>clarify.respond</span>
      <span>sudo.respond</span>
      <span>secret.respond</span>
    </div>
  </section>
  <section class="agent-context-card card">
    <h3>Co-work boards</h3>
    <p>Cron, kanban, and curator surfaces stay durable panes.</p>
    <div class="tc-display-terminal">cron.ready<br>kanban.ready<br>curator.ready</div>
  </section>
</div>"#,
        email = esc_html(&user.email),
        tenant = esc_html(&user.tenant_id),
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
        assert!(html.contains("passvault"));
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
