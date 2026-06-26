use crate::config;
use crate::lib::html::esc_html;
use crate::middleware::auth::AuthUser;
use crate::routes::agent::{AgentOutcome, MAX_PROMPT_CHARS};

pub const AGENT_OS_STYLES: &str = r#"
    :root {
      --bg: #f6f1e8;
      --surface: rgba(255, 252, 246, .86);
      --surface-2: rgba(246, 239, 229, .9);
      --primary: #2b2118;
      --secondary: #8a4b2a;
      --accent: #c15f3c;
      --accent-2: #7b8c6c;
      --muted: rgba(75, 63, 51, .08);
      --border: rgba(75, 63, 51, .16);
      --border-light: rgba(75, 63, 51, .1);
      --ink: #2b2118;
      --ink-muted: #6d6258;
      --ink-faint: #978b80;
      --shadow: 0 24px 70px rgba(61, 45, 31, .14), inset 0 1px 0 rgba(255,255,255,.7);
      --shadow-soft: 8px 12px 28px rgba(84, 62, 43, .08), -8px -10px 28px rgba(255,255,255,.55);
    }
    body {
      padding: 1.15rem;
      background:
        linear-gradient(180deg, rgba(255,255,255,.65), rgba(255,255,255,.1)),
        radial-gradient(circle at 12% 8%, rgba(193,95,60,.12), transparent 32%),
        #f6f1e8;
    }
    .console-wrap {
      width: min(1480px, 100%);
    }
    .console-topbar {
      margin-bottom: .7rem;
      color: var(--ink);
    }
    .console-dot {
      color: var(--ink-muted);
      border-color: var(--border);
      background: rgba(255,255,255,.55);
      box-shadow: inset 0 1px 0 rgba(255,255,255,.65);
    }
    .console-dot::before {
      background: var(--accent-2);
      box-shadow: 0 0 0 3px rgba(123,140,108,.12);
    }
    .console-card {
      border-radius: 18px;
      border-color: var(--border);
      background:
        linear-gradient(180deg, rgba(255,255,255,.78), rgba(255,255,255,.54)),
        rgba(246,241,232,.88);
      box-shadow: var(--shadow), var(--shadow-soft);
      backdrop-filter: blur(22px) saturate(140%);
    }
    .console-card::before { display: none; }
    .agent-os {
      min-height: min(82vh, 980px);
      display: grid;
      grid-template-columns: 248px minmax(0, 1fr) 312px;
      color: var(--ink);
      background: rgba(250, 246, 238, .7);
    }
    .agent-sidebar,
    .agent-context {
      min-width: 0;
      background: rgba(246, 239, 229, .74);
    }
    .agent-sidebar {
      display: grid;
      grid-template-rows: auto auto 1fr auto;
      gap: .8rem;
      padding: .8rem;
      border-right: 1px solid var(--border);
    }
    .agent-context {
      padding: .9rem;
      border-left: 1px solid var(--border);
    }
    .agent-main {
      min-width: 0;
      display: grid;
      grid-template-rows: auto minmax(0, 1fr) auto;
      background: rgba(255, 252, 246, .72);
    }
    .agent-product {
      display: flex;
      align-items: center;
      gap: .65rem;
      padding: .2rem .15rem .55rem;
    }
    .agent-avatar {
      width: 34px;
      height: 34px;
      border: 1px solid rgba(138,75,42,.24);
      border-radius: 10px;
      display: inline-grid;
      place-items: center;
      color: var(--secondary);
      background: linear-gradient(145deg, rgba(255,255,255,.75), rgba(236,224,209,.68));
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      font-size: .78rem;
      box-shadow: inset 0 1px 0 rgba(255,255,255,.8), 6px 8px 18px rgba(84,62,43,.08);
      flex: 0 0 auto;
    }
    .agent-brand {
      min-width: 0;
      color: var(--primary);
      font-size: .92rem;
      font-weight: 700;
      line-height: 1.15;
    }
    .agent-brand small {
      display: block;
      margin-top: .14rem;
      color: var(--ink-faint);
      font-size: .7rem;
      font-weight: 500;
    }
    .agent-meta,
    .agent-tabs,
    .agent-actions {
      display: flex;
      flex-wrap: wrap;
      gap: .45rem;
      align-items: center;
    }
    .agent-meta { justify-content: flex-start; }
    .agent-pill,
    .agent-tab,
    .agent-state {
      border: 1px solid var(--border);
      border-radius: 999px;
      color: var(--ink-muted);
      background: rgba(255,255,255,.58);
      padding: .28rem .54rem;
      font: .7rem/1.2 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      max-width: 100%;
      overflow-wrap: anywhere;
    }
    .agent-pill-live,
    .agent-tab-active {
      color: var(--secondary);
      border-color: rgba(193,95,60,.28);
      background: rgba(193,95,60,.08);
    }
    .agent-os .btn-primary {
      background: #2f261f;
      color: #fffaf2;
      border: 1px solid rgba(47,38,31,.12);
      box-shadow: 0 8px 18px rgba(47,38,31,.16);
    }
    .agent-os .btn-primary:hover {
      box-shadow: 0 10px 24px rgba(47,38,31,.2);
    }
    .agent-modebar {
      display: grid;
      gap: .35rem;
    }
    .agent-mode {
      min-height: 38px;
      display: flex;
      align-items: center;
      gap: .55rem;
      border: 1px solid transparent;
      border-radius: 10px;
      padding: .45rem .55rem;
      color: var(--ink-muted);
      background: transparent;
      text-decoration: none;
      font-size: .86rem;
      font-weight: 600;
    }
    .agent-mode::before {
      content: attr(data-icon);
      width: 1.25rem;
      color: var(--ink-faint);
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      font-size: .78rem;
      text-align: center;
    }
    .agent-mode-active {
      color: var(--primary);
      border-color: rgba(138,75,42,.16);
      background: rgba(255,255,255,.72);
      box-shadow: inset 0 1px 0 rgba(255,255,255,.75), 0 8px 20px rgba(84,62,43,.06);
    }
    .agent-section {
      padding: .75rem 0 0;
      border-top: 1px solid var(--border-light);
    }
    .agent-section:first-child { border-top: 0; padding-top: 0; }
    .agent-section h2,
    .agent-section h3 {
      color: var(--ink-faint);
      font-size: .68rem;
      letter-spacing: .06em;
      text-transform: uppercase;
      margin: 0 0 .52rem;
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-nav {
      display: grid;
      gap: .32rem;
    }
    .agent-nav a,
    .agent-nav span {
      min-height: 32px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: .6rem;
      border: 1px solid transparent;
      border-radius: 9px;
      padding: .42rem .48rem;
      color: var(--ink-muted);
      text-decoration: none;
      font-size: .8rem;
      background: transparent;
    }
    .agent-nav a:hover {
      color: var(--primary);
      border-color: rgba(138,75,42,.14);
      background: rgba(255,255,255,.52);
    }
    .agent-nav .active {
      color: var(--primary);
      border-color: rgba(138,75,42,.16);
      background: rgba(255,255,255,.66);
    }
    .agent-count {
      color: var(--ink-faint);
      font: .68rem/1 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .agent-new-task {
      width: 100%;
      display: inline-flex;
      align-items: center;
      justify-content: flex-start;
      border: 1px solid rgba(138,75,42,.18);
      background: rgba(255,255,255,.68);
      color: var(--secondary);
      border-radius: 10px;
      padding: .55rem .65rem;
      text-decoration: none;
      font-size: .86rem;
      font-weight: 700;
      box-shadow: inset 0 1px 0 rgba(255,255,255,.75);
    }
    .agent-new-task:hover {
      color: var(--primary);
      background: rgba(255,255,255,.86);
    }
    .agent-footnote {
      color: var(--ink-faint);
      font-size: .72rem;
      line-height: 1.45;
      margin: .2rem .1rem 0;
    }
    .agent-head {
      display: grid;
      gap: .75rem;
      padding: 1rem 1.15rem .85rem;
      border-bottom: 1px solid var(--border);
      background: rgba(255,255,255,.58);
    }
    .agent-head-top {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 1rem;
    }
    .agent-title {
      color: var(--primary);
      font-size: 1.08rem;
      font-weight: 700;
      letter-spacing: 0;
      margin: 0 0 .2rem;
    }
    .agent-subtitle {
      margin: 0;
      color: var(--ink-muted);
      font-size: .82rem;
      line-height: 1.5;
    }
    .agent-timeline {
      min-width: 0;
      display: grid;
      align-content: start;
      gap: .78rem;
      padding: 1rem 1.15rem 1.1rem;
      overflow: visible;
    }
    .agent-event,
    .agent-composer,
    .agent-context-card {
      min-width: 0;
      border: 1px solid var(--border);
      border-radius: 12px;
      background: rgba(255,255,255,.72);
      box-shadow: inset 0 1px 0 rgba(255,255,255,.72), 0 10px 28px rgba(84,62,43,.06);
    }
    .agent-event { padding: .78rem .88rem; }
    .agent-event-user {
      width: min(760px, 86%);
      justify-self: end;
      border-color: rgba(123,140,108,.24);
      background: rgba(244, 249, 239, .78);
    }
    .agent-event-assistant {
      width: min(820px, 92%);
      justify-self: start;
    }
    .agent-event-tool {
      width: min(760px, 88%);
      justify-self: start;
      background: rgba(246,239,229,.72);
    }
    .agent-event-error {
      border-color: var(--error-border);
      background: rgba(127, 29, 29, .08);
    }
    .agent-event-head {
      display: flex;
      justify-content: space-between;
      gap: .75rem;
      align-items: center;
      margin-bottom: .56rem;
    }
    .agent-channel {
      color: var(--ink-faint);
      font: .68rem/1 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      letter-spacing: .05em;
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
      font: .88rem/1.62 "DM Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    .agent-event-error pre { color: var(--error-ink); }
    .agent-tool-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: .45rem;
    }
    .agent-kv {
      border: 1px solid var(--border-light);
      border-radius: 10px;
      padding: .48rem .52rem;
      background: rgba(255,255,255,.55);
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
      padding: .72rem;
      background: rgba(255,255,255,.82);
    }
    .agent-composer label {
      color: var(--ink-faint);
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
      font-size: .7rem;
      text-transform: uppercase;
      letter-spacing: .06em;
    }
    .agent-composer textarea {
      min-height: 118px;
      border-radius: 11px;
      margin-top: .35rem;
      background: rgba(250, 246, 238, .75);
      color: var(--ink);
      border-color: var(--border);
    }
    .agent-composer-footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: .75rem;
      margin-top: .62rem;
    }
    .agent-hint {
      color: var(--ink-faint);
      font-size: .78rem;
      line-height: 1.45;
      margin: 0;
    }
    .agent-context-grid {
      display: grid;
      gap: .72rem;
    }
    .agent-context-card {
      padding: .78rem;
      background: rgba(255,255,255,.6);
    }
    .agent-context-card h3 {
      color: var(--primary);
      font-size: .82rem;
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
    .agent-progress {
      display: grid;
      gap: .48rem;
      margin-top: .54rem;
    }
    .agent-progress-row {
      display: flex;
      align-items: center;
      gap: .5rem;
      color: var(--ink-muted);
      font-size: .78rem;
    }
    .agent-progress-dot {
      width: 1rem;
      height: 1rem;
      display: inline-grid;
      place-items: center;
      border: 1px solid rgba(123,140,108,.35);
      border-radius: 999px;
      color: var(--accent-2);
      font-size: .62rem;
      background: rgba(244,249,239,.78);
    }
    .agent-request {
      display: grid;
      gap: .38rem;
      margin-top: .5rem;
    }
    .agent-request span {
      border: 1px solid var(--border-light);
      border-radius: 9px;
      padding: .38rem .46rem;
      color: var(--ink-muted);
      background: rgba(255,255,255,.48);
      font: .72rem/1.3 ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    @media (max-width: 1120px) {
      .agent-os { grid-template-columns: 220px minmax(0, 1fr); }
      .agent-context {
        grid-column: 1 / -1;
        border-left: 0;
        border-top: 1px solid var(--border);
      }
      .agent-context-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    @media (max-width: 760px) {
      .agent-os { grid-template-columns: 1fr; }
      .agent-sidebar,
      .agent-context {
        border: 0;
        border-bottom: 1px solid var(--border);
      }
      .agent-context { border-top: 1px solid var(--border); }
      .agent-head-top { flex-direction: column; }
      .agent-meta { justify-content: flex-start; }
      .agent-tool-grid,
      .agent-context-grid { grid-template-columns: 1fr; }
      .agent-event,
      .agent-event-user,
      .agent-event-tool,
      .agent-event-assistant { width: 100%; }
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
    <div class="agent-product">
      <div class="agent-avatar" aria-hidden="true">{initials}</div>
      <div class="agent-brand">AkurAI-RustAgent<small>olibuijr workspace</small></div>
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
        <span>Memory <b class="agent-count">context</b></span>
        <span>Notes <b class="agent-count">source</b></span>
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
    <p class="agent-footnote">The web surface uses the stable HTTP gateway today. Streaming events and receipts will attach here when the gateway exposes them.</p>
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
          <p class="agent-subtitle">Ask the tenant agent to inspect, edit, deploy, or organize work. Runs stay scoped to {email}.</p>
        </div>
        <div class="agent-meta" aria-label="Runtime metadata">
          <span class="agent-pill agent-pill-live">gateway stable</span>
          <span class="agent-pill">{provider}</span>
          <span class="agent-pill">{model}</span>
        </div>
      </div>
      <div class="agent-tabs" aria-label="Channels">
        <span class="agent-tab agent-tab-active">chat</span>
        <span class="agent-tab">analysis</span>
        <span class="agent-tab">tools</span>
        <span class="agent-tab">approvals</span>
        <span class="agent-tab">final</span>
      </div>
    </header>

    <div class="agent-timeline" aria-live="polite">
      {timeline}
    </div>

    <form class="agent-composer" method="post" action="" aria-label="Submit command to RustAgent">
      <input type="hidden" name="_csrf" value="{csrf}">
      <label for="prompt">Message AkurAI-RustAgent</label>
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
    r#"<article class="agent-event agent-event-assistant" data-channel="system" data-kind="ready">
  <div class="agent-event-head">
    <span class="agent-channel">AkurAI-RustAgent</span>
    <span class="agent-time">awaiting input</span>
  </div>
  <pre>I am ready to work in this tenant workspace. I can inspect code, summarize context, plan work, and run through the stable Rust gateway. Memory, notes, passvault, cron, kanban, and curator stay available as first-class panes.</pre>
</article>
<article class="agent-event agent-event-tool" data-channel="approval" data-kind="empty">
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
            r#"<article class="agent-event agent-event-tool" data-channel="tool_call" data-kind="gateway.query" data-tool-call-id="{tool_call_id}">
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
        r#"<article class="agent-event agent-event-user" data-channel="user" data-kind="message">
  <div class="agent-event-head">
    <span class="agent-channel">you</span>
    <span class="agent-time">submitted</span>
  </div>
  <pre>{prompt}</pre>
</article>
{tool_event}
<article class="{status_class} agent-event-assistant" data-channel="final" data-kind="{status}">
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
  <section class="agent-context-card">
    <h3>Run progress</h3>
    <p>The HTTP gateway is stable. Rich event streaming and approvals will attach to this lane when exposed by RustAgent.</p>
    <div class="agent-progress">
      <div class="agent-progress-row"><span class="agent-progress-dot">1</span><span>Tenant session scoped</span></div>
      <div class="agent-progress-row"><span class="agent-progress-dot">2</span><span>Gateway query ready</span></div>
      <div class="agent-progress-row"><span class="agent-progress-dot">3</span><span>Tool receipts reserved</span></div>
    </div>
  </section>
  <section class="agent-context-card">
    <h3>Artifacts</h3>
    <p>Diffs, previews, files, tables, and generated outputs will render here instead of being buried in chat text.</p>
    <ul>
      <li>Code previews</li>
      <li>Tool output cards</li>
      <li>Curator reports</li>
    </ul>
  </section>
  <section class="agent-context-card">
    <h3>Context</h3>
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
      <li>Memory context</li>
      <li>Durable notes</li>
      <li>Sealed passvault</li>
    </ul>
  </section>
  <section class="agent-context-card">
    <h3>Pending confirmations</h3>
    <p>No active asks. Future receipts map directly to the matching response method.</p>
    <div class="agent-request">
      <span>approval.respond</span>
      <span>clarify.respond</span>
      <span>sudo.respond</span>
      <span>secret.respond</span>
    </div>
  </section>
  <section class="agent-context-card">
    <h3>Co-work boards</h3>
    <p>Cron, kanban, and curator surfaces stay durable panes, not plain chat transcripts.</p>
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
        assert!(html.contains("tools"));
    }

    #[test]
    fn body_escapes_prompt() {
        let html = agent_body(&user(), "csrf", "<script>", None);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
