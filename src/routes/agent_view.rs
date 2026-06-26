use crate::config;
use crate::lib::html::esc_html;
use crate::middleware::auth::AuthUser;
use crate::routes::agent::{AgentOutcome, MAX_PROMPT_CHARS};

pub const AGENT_OS_STYLES: &str = r#"
    .agent-os {
      min-height: min(78vh, 980px);
      display: grid;
      grid-template-columns: 232px minmax(0, 1fr) 284px;
      background:
        linear-gradient(180deg, rgba(255,255,255,.045), rgba(255,255,255,.015)),
        rgba(3, 7, 11, .45);
    }
    .agent-sidebar,
    .agent-context {
      padding: 1rem;
      min-width: 0;
      background: rgba(3, 7, 11, .34);
    }
    .agent-sidebar { border-right: 1px solid var(--border-light); }
    .agent-context { border-left: 1px solid var(--border-light); }
    .agent-main {
      min-width: 0;
      display: grid;
      grid-template-rows: auto minmax(0, 1fr) auto;
      background:
        linear-gradient(rgba(255,255,255,.028) 1px, transparent 1px),
        linear-gradient(90deg, rgba(255,255,255,.024) 1px, transparent 1px);
      background-size: 32px 32px;
    }
    .agent-head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 1rem;
      padding: 1rem 1.15rem;
      border-bottom: 1px solid var(--border-light);
      background: rgba(255,255,255,.026);
    }
    .agent-title {
      color: var(--primary);
      font-size: 1rem;
      font-weight: 700;
      letter-spacing: 0;
      margin: 0 0 .25rem;
    }
    .agent-subtitle {
      margin: 0;
      color: var(--ink-muted);
      font-size: .82rem;
      line-height: 1.5;
    }
    .agent-avatar {
      width: 42px;
      height: 42px;
      border: 1px solid rgba(103,232,249,.38);
      border-radius: 14px;
      display: inline-grid;
      place-items: center;
      color: var(--accent);
      background: linear-gradient(145deg, rgba(103,232,249,.14), rgba(183,243,107,.08));
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      font-size: .82rem;
      box-shadow: inset 0 1px 0 rgba(255,255,255,.08), 8px 8px 24px rgba(0,0,0,.22);
      flex: 0 0 auto;
    }
    .agent-meta,
    .agent-tabs,
    .agent-actions {
      display: flex;
      flex-wrap: wrap;
      gap: .45rem;
      align-items: center;
    }
    .agent-meta { justify-content: flex-end; }
    .agent-pill,
    .agent-tab,
    .agent-state {
      border: 1px solid var(--border-light);
      border-radius: 999px;
      color: var(--ink-muted);
      background: rgba(255,255,255,.048);
      padding: .32rem .58rem;
      font: .72rem/1.2 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      max-width: 100%;
      overflow-wrap: anywhere;
    }
    .agent-pill-live,
    .agent-tab-active {
      color: var(--accent);
      border-color: rgba(183,243,107,.34);
      background: rgba(183,243,107,.07);
    }
    .agent-section {
      padding: .95rem 0;
      border-top: 1px solid var(--border-light);
    }
    .agent-section:first-child { border-top: 0; padding-top: 0; }
    .agent-section h2,
    .agent-section h3 {
      color: var(--primary);
      font-size: .74rem;
      letter-spacing: .08em;
      text-transform: uppercase;
      margin: 0 0 .7rem;
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-nav {
      display: grid;
      gap: .45rem;
    }
    .agent-nav a,
    .agent-nav span {
      min-height: 34px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: .6rem;
      border: 1px solid transparent;
      border-radius: 8px;
      padding: .48rem .55rem;
      color: var(--ink-muted);
      text-decoration: none;
      font-size: .82rem;
      background: rgba(255,255,255,.025);
    }
    .agent-nav a:hover {
      color: var(--primary);
      border-color: rgba(103,232,249,.26);
      background: rgba(103,232,249,.07);
    }
    .agent-nav .active {
      color: var(--primary);
      border-color: rgba(183,243,107,.28);
      background: linear-gradient(135deg, rgba(103,232,249,.12), rgba(183,243,107,.08));
    }
    .agent-count {
      color: var(--ink-faint);
      font: .68rem/1 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-timeline {
      min-width: 0;
      display: grid;
      align-content: start;
      gap: .85rem;
      padding: 1rem 1.15rem;
      overflow: visible;
    }
    .agent-event,
    .agent-composer,
    .agent-context-card {
      min-width: 0;
      border: 1px solid var(--border-light);
      border-radius: 8px;
      background: rgba(6, 10, 15, .72);
      box-shadow: inset 0 1px 0 rgba(255,255,255,.05), 10px 14px 30px rgba(0,0,0,.2);
    }
    .agent-event { padding: .9rem; }
    .agent-event-error {
      border-color: var(--error-border);
      background: rgba(69, 10, 10, .24);
    }
    .agent-event-head {
      display: flex;
      justify-content: space-between;
      gap: .75rem;
      align-items: center;
      margin-bottom: .7rem;
    }
    .agent-channel {
      color: var(--secondary);
      font: .7rem/1 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      letter-spacing: .08em;
      text-transform: uppercase;
    }
    .agent-time {
      color: var(--ink-faint);
      font: .7rem/1 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-event pre,
    .agent-code {
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      color: var(--ink);
      font: .88rem/1.6 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-event-error pre { color: var(--error-ink); }
    .agent-tool-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: .55rem;
    }
    .agent-kv {
      border: 1px solid var(--border-light);
      border-radius: 8px;
      padding: .55rem;
      background: rgba(255,255,255,.03);
      min-width: 0;
    }
    .agent-kv dt {
      margin: 0 0 .25rem;
      color: var(--ink-faint);
      font-size: .68rem;
    }
    .agent-kv dd {
      margin: 0;
      color: var(--ink);
      font: .76rem/1.35 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      overflow-wrap: anywhere;
    }
    .agent-composer {
      margin: 0 1.15rem 1rem;
      padding: .85rem;
      background: linear-gradient(180deg, rgba(255,255,255,.052), rgba(255,255,255,.022));
    }
    .agent-composer label {
      color: var(--accent);
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      font-size: .78rem;
    }
    .agent-composer textarea {
      min-height: 132px;
      border-radius: 8px;
      margin-top: .35rem;
    }
    .agent-composer-footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: .75rem;
      margin-top: .7rem;
    }
    .agent-hint {
      color: var(--ink-faint);
      font-size: .78rem;
      line-height: 1.45;
      margin: 0;
    }
    .agent-context-grid {
      display: grid;
      gap: .7rem;
    }
    .agent-context-card { padding: .75rem; }
    .agent-context-card h3 {
      color: var(--primary);
      font-size: .8rem;
      margin: 0 0 .35rem;
      letter-spacing: 0;
      text-transform: none;
    }
    .agent-context-card p,
    .agent-context-card li {
      color: var(--ink-muted);
      font-size: .8rem;
      line-height: 1.5;
      margin: 0;
    }
    .agent-context-card ul {
      display: grid;
      gap: .35rem;
      list-style: none;
      margin: .5rem 0 0;
    }
    .agent-request {
      display: grid;
      gap: .45rem;
      margin-top: .55rem;
    }
    .agent-request span {
      border: 1px solid var(--border-light);
      border-radius: 8px;
      padding: .42rem .48rem;
      color: var(--ink-muted);
      background: rgba(255,255,255,.025);
      font: .72rem/1.3 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    @media (max-width: 1120px) {
      .agent-os { grid-template-columns: 210px minmax(0, 1fr); }
      .agent-context {
        grid-column: 1 / -1;
        border-left: 0;
        border-top: 1px solid var(--border-light);
      }
      .agent-context-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    @media (max-width: 760px) {
      .agent-os { grid-template-columns: 1fr; }
      .agent-sidebar,
      .agent-context {
        border: 0;
        border-bottom: 1px solid var(--border-light);
      }
      .agent-context { border-top: 1px solid var(--border-light); }
      .agent-head { flex-direction: column; }
      .agent-meta { justify-content: flex-start; }
      .agent-tool-grid,
      .agent-context-grid { grid-template-columns: 1fr; }
      .agent-composer-footer { align-items: stretch; flex-direction: column; }
      .agent-actions { justify-content: space-between; }
      .agent-actions .btn { width: 100%; }
    }
"#;

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
        r#"<section class="agent-os" aria-label="AkurAI-RustAgent workspace">
  <aside class="agent-sidebar" aria-label="Agent workspace navigation">
    <div class="agent-section">
      <div class="agent-avatar" aria-hidden="true">{initials}</div>
      <h2>Main tenant agent</h2>
      <nav class="agent-nav" aria-label="Workspace">
        <span class="active">Run timeline <b class="agent-count">live</b></span>
        <span>Memory <b class="agent-count">persistent</b></span>
        <span>Notes <b class="agent-count">vaulted</b></span>
        <span>Passvault <b class="agent-count">sealed</b></span>
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
    <div class="agent-section">
      <h2>Account</h2>
      <nav class="agent-nav" aria-label="Account links">
        <a href="/account">AkurAI ID <b class="agent-count">open</b></a>
      </nav>
    </div>
  </aside>

  <section class="agent-main" aria-label="Agent run">
    <header class="agent-head">
      <div>
        <h1 class="agent-title">AkurAI-RustAgent</h1>
        <p class="agent-subtitle">Authenticated workspace for {email}. Gateway runs stay scoped to this tenant session.</p>
        <div class="agent-tabs" aria-label="Channels">
          <span class="agent-tab agent-tab-active">commentary</span>
          <span class="agent-tab">tool_call</span>
          <span class="agent-tab">approval</span>
          <span class="agent-tab">question</span>
          <span class="agent-tab">final</span>
        </div>
      </div>
      <div class="agent-meta" aria-label="Runtime metadata">
        <span class="agent-pill agent-pill-live">gateway stable</span>
        <span class="agent-pill">{provider}</span>
        <span class="agent-pill">{model}</span>
      </div>
    </header>

    <div class="agent-timeline" aria-live="polite">
      {timeline}
    </div>

    <form class="agent-composer" method="post" action="" aria-label="Submit command to RustAgent">
      <input type="hidden" name="_csrf" value="{csrf}">
      <label for="prompt">Command</label>
      <textarea id="prompt" name="prompt" maxlength="{max_prompt}" required spellcheck="false" autocomplete="off" placeholder="Ask Rust Agent to inspect, plan, edit, deploy, or operate your workspace...">{prompt}</textarea>
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
        email = esc_html(&user.email),
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
        r#"<section class="agent-os" aria-label="AkurAI-RustAgent workspace">
  <section class="agent-main" style="grid-column: 1 / -1;">
    <div class="agent-timeline">
      <article class="agent-event agent-event-error">
        <div class="agent-event-head">
          <span class="agent-channel">system.error</span>
          <span class="agent-time">access denied</span>
        </div>
        <pre>{email} is not enabled for the AkurAI-RustAgent workspace.</pre>
      </article>
    </div>
  </section>
</section>"#,
        email = esc_html(&user.email),
    )
}

fn render_ready_timeline() -> String {
    r#"<article class="agent-event" data-channel="system" data-kind="ready">
  <div class="agent-event-head">
    <span class="agent-channel">system.ready</span>
    <span class="agent-time">awaiting input</span>
  </div>
  <pre>AkurAI-RustAgent is ready. The next command will run through the stable Rust gateway and return as a structured timeline item.</pre>
</article>
<article class="agent-event" data-channel="approval" data-kind="empty">
  <div class="agent-event-head">
    <span class="agent-channel">approval.lane</span>
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
            r#"<article class="agent-event" data-channel="tool_call" data-kind="gateway.query" data-tool-call-id="{tool_call_id}">
  <div class="agent-event-head">
    <span class="agent-channel">tool_call.gateway.query</span>
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
        r#"<article class="agent-event" data-channel="user" data-kind="message">
  <div class="agent-event-head">
    <span class="agent-channel">user.command</span>
    <span class="agent-time">submitted</span>
  </div>
  <pre>{prompt}</pre>
</article>
{tool_event}
<article class="{status_class}" data-channel="final" data-kind="{status}">
  <div class="agent-event-head">
    <span class="agent-channel">{channel}</span>
    <span class="agent-time">{latency} ms</span>
  </div>
  <pre>{response}</pre>
</article>"#,
        prompt = esc_html(prompt),
        tool_event = tool_event,
        status_class = status_class,
        channel = if outcome.ok {
            "final.response"
        } else {
            "error.gateway"
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
  <section class="agent-context-card">
    <h3>Identity</h3>
    <p>{email}</p>
    <ul>
      <li class="mono">{tenant}</li>
      <li class="mono">{scope}</li>
      <li class="mono">{session}</li>
    </ul>
  </section>
  <section class="agent-context-card">
    <h3>Persistent workspace</h3>
    <p>Memory, notes, and passvault stay RustAgent-owned and tenant scoped.</p>
    <ul>
      <li>Memory: first-class context</li>
      <li>Notes: durable operator knowledge</li>
      <li>Passvault: sealed credentials</li>
    </ul>
  </section>
  <section class="agent-context-card">
    <h3>Pending asks</h3>
    <p>No active confirmations. Future receipts map directly to the matching response method.</p>
    <div class="agent-request">
      <span>approval.respond</span>
      <span>clarify.respond</span>
      <span>sudo.respond</span>
      <span>secret.respond</span>
    </div>
  </section>
  <section class="agent-context-card">
    <h3>Automation ecosystem</h3>
    <p>Cron, kanban, and curator surfaces are reserved as durable panes, not plain chat transcripts.</p>
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
