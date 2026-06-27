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
    let panels = render_panel_templates(user, &cfg.agent_provider, &cfg.agent_model);

    format!(
        r#"<section class="agent-os chat-root" aria-label="AkurAI Agent" data-agent-ui data-session="{session_id}">
  <aside class="agent-sidebar" aria-label="Workspace">
    <div class="agent-product">
      <div class="agent-avatar" aria-hidden="true">{initials}</div>
      <div class="agent-brand">AkurAI<small>Personal agent</small></div>
    </div>
    <a class="agent-new-task" href="/agent">New chat</a>
    <div class="agent-section">
      <h2>Workspace</h2>
      <nav class="agent-nav" aria-label="Workspace">
        <a class="active" href="/agent">Current conversation <b class="agent-count">now</b></a>
        <a href="/agent/kanban" data-panel-trigger="kanban">Kanban <b class="agent-count" data-kanban-nav-count>board</b></a>
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
        <p class="agent-subtitle">Personal agent</p>
      </div>
      <div class="agent-meta" aria-label="Runtime">
        <button type="button" data-panel-trigger="kanban">Kanban</button>
        <button type="button" data-panel-trigger="tools">Tools</button>
        <button type="button" data-panel-trigger="run">Run details</button>
      </div>
    </header>

    <div class="agent-timeline chat-thread" aria-live="polite">
      <section class="agent-tool-panel" data-agent-panel hidden></section>
      {timeline}
    </div>

    <form class="agent-composer chat-composer" method="post" action="/agent" aria-label="Message your agent">
      <input type="hidden" name="_csrf" value="{csrf}">
      <textarea id="prompt" name="prompt" maxlength="{max_prompt}" required spellcheck="false" autocomplete="off" aria-label="Message your agent" placeholder="Message your agent...">{prompt}</textarea>
      <div class="agent-composer-footer">
        <span class="agent-state" data-agent-status>Ready</span>
        <button type="submit" class="btn btn-primary">Run</button>
      </div>
    </form>
  </section>

  <aside class="agent-context" aria-label="Run details"></aside>
  <div class="agent-protocol" hidden data-session="{session_id}">
    analysis commentary final tool_call tool_result approval question edit artifact system error
    clarify.request approval.request sudo.request secret.request terminal.read.request
  </div>
  {panels}
</section>"#,
        initials = esc_html(&agent_initials(&user.email)),
        csrf = esc_html(csrf),
        max_prompt = MAX_PROMPT_CHARS,
        prompt = esc_html(prompt),
        session_id = esc_html(&session_id),
        timeline = timeline,
        panels = panels,
    )
}

pub(crate) fn forbidden_body(user: &AuthUser) -> String {
    format!(
        r#"<section class="agent-os chat-root" aria-label="AkurAI Agent">
  <section class="agent-main" style="grid-column: 1 / -1;">
    <div class="agent-timeline chat-thread">
      <article class="agent-event agent-event-error chat-message" data-channel="error">
        <div class="agent-event-head">
          <span class="agent-channel">system</span>
          <span class="agent-time">access denied</span>
        </div>
        <pre class="chat-message-content">{email} is not enabled for this agent.</pre>
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
  <p>Your agent is ready.</p>
  <div class="agent-suggestions" aria-label="Suggestions">
    <button type="button" data-agent-prompt="Inspect the current agent status and tell me the next concrete fixes.">Inspect agent</button>
    <button type="button" data-agent-prompt="Plan the next agent deploy. Include risks, checks, and rollback.">Plan a deploy</button>
    <button type="button" data-panel-trigger="tools">Open tools</button>
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
        channel = if outcome.ok { "agent" } else { "gateway error" },
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

fn render_panel_templates(user: &AuthUser, provider: &str, model: &str) -> String {
    let session_id = agent_session_id(user);
    let scope_id = agent_scope_id(user);

    format!(
        r#"<template data-panel-template="tools">
  <div class="agent-panel-head">
    <div><h2>Agent tools</h2><p>Use these when the conversation needs context, notes, credentials, or work queues.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-tool-launcher">
    <button type="button" data-panel-trigger="tasks"><b>Tasks</b><span>Plan, create, and review work.</span></button>
    <button type="button" data-panel-trigger="projects"><b>Projects</b><span>Open AkurAI workspaces.</span></button>
    <button type="button" data-panel-trigger="agy"><b>AGY</b><span>Use durable personal context.</span></button>
    <button type="button" data-panel-trigger="notes"><b>Notes</b><span>Save local working notes.</span></button>
    <button type="button" data-panel-trigger="passvault"><b>Passvault</b><span>Prepare confirmation-gated secrets.</span></button>
    <button type="button" data-panel-trigger="cron"><b>Cron</b><span>Draft scheduled checks.</span></button>
    <button type="button" data-panel-trigger="kanban"><b>Kanban</b><span>Shape active work into a board.</span></button>
    <button type="button" data-panel-trigger="curator"><b>Curator</b><span>Find bloat and unfinished UI.</span></button>
  </div>
</template>

<template data-panel-template="run">
  <div class="agent-panel-head">
    <div><h2>Current run</h2><p>{email}</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <dl class="agent-panel-grid">
    <div><dt>Provider</dt><dd>{provider}</dd></div>
    <div><dt>Model</dt><dd>{model}</dd></div>
    <div><dt>Scope</dt><dd>{scope}</dd></div>
    <div><dt>Session</dt><dd>{session}</dd></div>
  </dl>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Check the current agent run state and list any blocked or risky items.">Check run</button>
    <button type="button" data-agent-prompt="Before making changes, ask me for any confirmations this agent needs.">Ask confirmations</button>
  </div>
</template>

<template data-panel-template="tasks">
  <div class="agent-panel-head">
    <div><h2>Tasks</h2><p>Work queue for this tenant agent.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Review open work</b><span>Summarize active agent tasks and gaps.</span></article>
    <article><b>Create from chat</b><span>Turn the current conversation into a concrete task.</span></article>
    <article><b>Confirm before action</b><span>Collect approvals before destructive or credentialed steps.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Review my open agent tasks and group them by priority.">Review tasks</button>
    <button type="button" data-agent-prompt="Create a concise task plan from this conversation and wait for confirmation.">Create task</button>
  </div>
</template>

<template data-panel-template="projects">
  <div class="agent-panel-head">
    <div><h2>Projects</h2><p>AkurAI workspaces connected to this agent.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Agent Core</b><span>Gateway, CLI/TUI, and persistent AGY context.</span></article>
    <article><b>AkurAI IDP</b><span>Agent console, login, tenant routing, auth surface.</span></article>
    <article><b>AkurAI Framework</b><span>Shared theme registry and console components.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Inspect the agent core project and report the highest-impact incomplete UI/runtime work.">Inspect project</button>
    <button type="button" data-agent-prompt="Compare AkurAI IDP and agent core UI responsibilities and suggest the clean boundary.">Check boundary</button>
  </div>
</template>

<template data-panel-template="agy">
  <div class="agent-panel-head">
    <div><h2>AGY context</h2><p>Personal agent context, durable notes, and preferences.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Identity</b><span>Tenant agent for olibuijr@olibuijr.com.</span></article>
    <article><b>Unique features</b><span>Persistent AGY context, notes, and passvault-aware requests.</span></article>
    <article><b>Operating rule</b><span>Ask for confirmation when a tool action needs approval.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Load AGY context for this tenant and summarize the durable preferences that matter now.">Summarize AGY</button>
    <button type="button" data-agent-prompt="Use AGY context, notes, and passvault boundaries before planning the next step.">Use context</button>
  </div>
</template>

<template data-panel-template="notes">
  <div class="agent-panel-head">
    <div><h2>Notes</h2><p>Local working notes for this browser session.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <textarea class="agent-notes-editor" data-notes-editor placeholder="Write working notes for this agent..."></textarea>
  <div class="agent-panel-actions">
    <button type="button" data-save-notes>Save notes</button>
    <button type="button" data-use-notes>Use notes</button>
  </div>
  <p class="agent-panel-status" data-notes-status></p>
</template>

<template data-panel-template="passvault">
  <div class="agent-panel-head">
    <div><h2>Passvault</h2><p>Credential requests stay confirmation-gated.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Sealed by default</b><span>No credential is shown in the UI.</span></article>
    <article><b>Confirm first</b><span>The agent should ask before using a secret.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Prepare a passvault-backed request. Ask me exactly what secret or confirmation you need before using it.">Request secret</button>
    <button type="button" data-agent-prompt="Audit whether the next task needs passvault access. If yes, ask for confirmation first.">Check access</button>
  </div>
</template>

<template data-panel-template="cron">
  <div class="agent-panel-head">
    <div><h2>Cron</h2><p>Scheduled agent work.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Daily health</b><span>Check deploy state, gateway health, and parity drift.</span></article>
    <article><b>Follow-up sweep</b><span>Find stale tasks and unfinished UI surfaces.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Draft a cron schedule for agent health checks and ask before enabling anything.">Draft schedule</button>
  </div>
</template>

<template data-panel-template="kanban">
  <div class="agent-panel-head">
    <div><h2>Kanban</h2><p>Rust Agent board, tasks, claims, and project delivery state.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <section class="kanban-shell" data-kanban-board>
    <div class="kanban-toolbar">
      <label>Board <select data-kanban-board-select aria-label="Kanban board"></select></label>
      <label class="kanban-check"><input type="checkbox" data-kanban-include-done checked> Done</label>
      <button type="button" data-kanban-refresh>Refresh</button>
      <button type="button" data-kanban-reclaim>Reclaim stale</button>
      <button type="button" data-kanban-dispatch>Dry-run dispatch</button>
    </div>
    <form class="kanban-create" data-kanban-create>
      <input name="title" maxlength="160" required placeholder="Task title" aria-label="Task title">
      <input name="assignee" maxlength="80" placeholder="Assignee" aria-label="Assignee">
      <textarea name="description" maxlength="1200" placeholder="Description" aria-label="Description"></textarea>
      <button type="submit">Create</button>
    </form>
    <div class="kanban-metrics" data-kanban-metrics></div>
    <div class="kanban-columns" aria-label="Kanban columns">
      <section class="kanban-column" data-kanban-column="todo"><h3>Todo <b>0</b></h3><div></div></section>
      <section class="kanban-column" data-kanban-column="doing"><h3>Doing <b>0</b></h3><div></div></section>
      <section class="kanban-column" data-kanban-column="blocked"><h3>Blocked <b>0</b></h3><div></div></section>
      <section class="kanban-column" data-kanban-column="done"><h3>Done <b>0</b></h3><div></div></section>
    </div>
    <section class="kanban-detail" data-kanban-detail hidden></section>
    <p class="agent-panel-status" data-kanban-status></p>
  </section>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Review the Rust Agent kanban board, summarize blocked tasks, and propose the next project-management action.">Review board</button>
    <button type="button" data-agent-prompt="Create a practical agile plan from the active kanban work, including next task, owner, and verification step.">Plan sprint</button>
  </div>
</template>

<template data-panel-template="curator">
  <div class="agent-panel-head">
    <div><h2>Curator</h2><p>Cleanup and quality pass.</p></div>
    <button type="button" class="agent-panel-close" data-panel-close aria-label="Close">Close</button>
  </div>
  <div class="agent-panel-list">
    <article><b>Remove dead UI</b><span>Find controls that do not map to a real workflow.</span></article>
    <article><b>Tighten copy</b><span>Reduce status noise and internal implementation labels.</span></article>
    <article><b>Verify live</b><span>Use screenshots and smoke prompts after deploy.</span></article>
  </div>
  <div class="agent-panel-actions">
    <button type="button" data-agent-prompt="Act as curator: identify UI bloat, dead controls, and the smallest fixes to make this console feel finished.">Run curator</button>
  </div>
</template>"#,
        email = esc_html(&user.email),
        scope = esc_html(&scope_id),
        session = esc_html(&session_id),
        provider = esc_html(provider),
        model = esc_html(model),
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
        assert!(html.contains("AkurAI"));
        assert!(html.contains("Passvault"));
        assert!(html.contains("Kanban"));
        assert!(html.contains(r#"href="/agent/kanban""#));
        assert!(html.contains("data-kanban-board"));
        assert!(html.contains("data-kanban-create"));
        assert!(html.contains("data-panel-trigger=\"notes\""));
        assert!(html.contains("data-use-notes"));
        assert!(html.contains("data-agent-prompt"));
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
