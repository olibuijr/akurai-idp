const roots = document.querySelectorAll("[data-agent-ui]");

for (const root of roots) {
  const panel = root.querySelector("[data-agent-panel]");
  const timeline = root.querySelector(".agent-timeline");
  const form = root.querySelector("form.agent-composer");
  const prompt = root.querySelector("textarea[name='prompt']");
  const status = root.querySelector("[data-agent-status]");
  const submit = form?.querySelector("button[type='submit']");
  const session = root.dataset.session || "default";
  const notesKey = `akurai-agent-notes:${session}`;
  const csrf = form?.querySelector("input[name='_csrf']")?.value || "";
  let running = false;
  const toolPanels = new Set([
    "tasks",
    "projects",
    "agy",
    "notes",
    "passvault",
    "cron",
    "curator",
  ]);

  const setStatus = (text) => {
    if (status) status.textContent = text;
  };

  const setBusy = (busy) => {
    running = busy;
    form?.classList.toggle("agent-composer-busy", busy);
    if (submit) submit.disabled = busy;
  };

  const scrollToLatest = () => {
    if (!timeline) return;
    requestAnimationFrame(() => {
      timeline.scrollTop = timeline.scrollHeight;
    });
  };

  const removeReadyState = () => {
    timeline?.querySelector("[data-kind='ready']")?.remove();
  };

  const appendEvent = ({ role, channel, time, content, error = false }) => {
    if (!timeline) return null;
    removeReadyState();

    const article = document.createElement("article");
    article.className = "agent-event chat-message";
    article.dataset.channel = channel;
    article.dataset.kind = error ? "error" : "message";
    if (role === "user") {
      article.classList.add("agent-event-user", "chat-message-user");
    } else {
      article.classList.add("agent-event-assistant", "chat-message-assistant");
    }
    if (error) article.classList.add("agent-event-error");

    const head = document.createElement("div");
    head.className = "agent-event-head";
    const channelEl = document.createElement("span");
    channelEl.className = "agent-channel";
    channelEl.textContent = channel;
    const timeEl = document.createElement("span");
    timeEl.className = "agent-time";
    timeEl.textContent = time;
    head.append(channelEl, timeEl);

    const body = document.createElement("pre");
    body.className = "chat-message-content";
    body.textContent = content;

    article.append(head, body);
    timeline.append(article);
    scrollToLatest();
    return article;
  };

  const updateAssistant = (article, payload) => {
    if (!article) return;
    const ok = payload.ok !== false;
    article.classList.toggle("agent-event-error", !ok);
    article.dataset.kind = ok ? "complete" : "error";
    article.dataset.channel = ok ? "final" : "error";
    article.querySelector(".agent-channel").textContent = ok ? "agent" : "gateway error";
    article.querySelector(".agent-time").textContent = payload.latency_ms
      ? `${payload.latency_ms} ms`
      : ok
        ? "complete"
        : "error";
    article.querySelector(".chat-message-content").textContent =
      payload.response || "No response returned.";
    scrollToLatest();
  };

  const parseEvent = (block) => {
    let event = "message";
    let data = "";
    for (const line of block.split(/\r?\n/)) {
      if (line.startsWith("event:")) {
        event = line.slice(6).trim();
      } else if (line.startsWith("data:")) {
        data += line.slice(5).trimStart();
      }
    }
    if (!data) return null;
    try {
      return { event, payload: JSON.parse(data) };
    } catch {
      return { event: "error", payload: { ok: false, response: "Invalid stream event." } };
    }
  };

  const consumeStream = async (response, onEvent) => {
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { value, done } = await reader.read();
      buffer += decoder.decode(value || new Uint8Array(), { stream: !done });

      let boundary = buffer.indexOf("\n\n");
      while (boundary >= 0) {
        const block = buffer.slice(0, boundary).trim();
        buffer = buffer.slice(boundary + 2);
        const parsed = parseEvent(block);
        if (parsed) onEvent(parsed.event, parsed.payload);
        boundary = buffer.indexOf("\n\n");
      }

      if (done) {
        const parsed = parseEvent(buffer.trim());
        if (parsed) onEvent(parsed.event, parsed.payload);
        break;
      }
    }
  };

  const submitStream = async (event) => {
    if (!form || !prompt || !window.ReadableStream || !window.TextDecoder) return;
    event.preventDefault();
    if (running) return;

    const text = prompt.value.trim();
    if (!text) {
      setStatus("Prompt is empty");
      prompt.focus();
      return;
    }

    const body = new URLSearchParams();
    body.set("prompt", text);
    body.set("_csrf", csrf);

    appendEvent({ role: "user", channel: "you", time: "submitted", content: text });
    const assistant = appendEvent({
      role: "assistant",
      channel: "agent",
      time: "starting",
      content: "Starting...",
    });

    setBusy(true);
    setStatus("Running");
    prompt.value = "";

    try {
      const response = await fetch("/agent/stream", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          Accept: "text/event-stream",
          "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
          "X-CSRF-Token": csrf,
        },
        body: body.toString(),
      });

      if (!response.ok || !response.body) {
        throw new Error(`Stream failed with HTTP ${response.status}`);
      }

      await consumeStream(response, (name, payload) => {
        if (name === "start") {
          setStatus("Running");
          assistant.querySelector(".agent-time").textContent = "running";
          assistant.querySelector(".chat-message-content").textContent = "Thinking...";
          return;
        }
        updateAssistant(assistant, payload);
        setStatus(payload.ok === false ? "Error" : "Complete");
      });
    } catch (error) {
      updateAssistant(assistant, {
        ok: false,
        response: error instanceof Error ? error.message : "Agent stream failed.",
      });
      prompt.value = text;
      setStatus("Error");
    } finally {
      setBusy(false);
      prompt.focus();
    }
  };

  const activate = (key) => {
    const activeKey = toolPanels.has(key) ? "tools" : key;
    root.querySelectorAll("[data-panel-trigger]").forEach((button) => {
      button.classList.toggle("active", button.dataset.panelTrigger === activeKey);
      button.classList.toggle("agent-mode-active", button.dataset.panelTrigger === activeKey);
    });
  };

  const fillPrompt = (text) => {
    if (!prompt || !text) return;
    prompt.value = text;
    prompt.focus();
    prompt.setSelectionRange(prompt.value.length, prompt.value.length);
    setStatus("Ready to run");
  };

  const bindPanel = () => {
    panel.querySelector("[data-panel-close]")?.addEventListener("click", () => {
      panel.hidden = true;
      activate("chat");
      setStatus("Ready");
    });

    panel.querySelectorAll("[data-agent-prompt]").forEach((button) => {
      button.addEventListener("click", () => fillPrompt(button.dataset.agentPrompt));
    });

    panel.querySelectorAll("[data-panel-trigger]").forEach((button) => {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        openPanel(button.dataset.panelTrigger);
      });
    });

    const notes = panel.querySelector("[data-notes-editor]");
    const notesStatus = panel.querySelector("[data-notes-status]");
    if (notes) {
      notes.value = localStorage.getItem(notesKey) || "";
      panel.querySelector("[data-save-notes]")?.addEventListener("click", () => {
        localStorage.setItem(notesKey, notes.value);
        if (notesStatus) notesStatus.textContent = "Saved locally";
        setStatus("Notes saved");
      });
      panel.querySelector("[data-use-notes]")?.addEventListener("click", () => {
        const text = notes.value.trim();
        fillPrompt(
          text
            ? `Use these local notes as context and propose the next concrete action:\n\n${text}`
            : "No local notes are saved yet. Ask me what to capture before continuing.",
        );
      });
    }

    const kanban = panel.querySelector("[data-kanban-board]");
    if (kanban) initKanbanPanel(kanban);
  };

  const openPanel = (key) => {
    if (key === "chat") {
      panel.hidden = true;
      activate("chat");
      setStatus("Ready");
      prompt?.focus();
      return;
    }
    const template = root.querySelector(`template[data-panel-template="${key}"]`);
    if (!template || !panel) return;
    panel.replaceChildren(template.content.cloneNode(true));
    panel.hidden = false;
    activate(key);
    bindPanel();
    setStatus(`${key} open`);
  };

  root.querySelectorAll("[data-panel-trigger]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      openPanel(button.dataset.panelTrigger);
    });
  });

  root.querySelectorAll("[data-agent-prompt]").forEach((button) => {
    button.addEventListener("click", () => fillPrompt(button.dataset.agentPrompt));
  });

  form?.addEventListener("submit", submitStream);

  if (location.pathname === "/agent/kanban") {
    openPanel("kanban");
  }

  async function kanbanFetch(path, options = {}) {
    const headers = {
      Accept: "application/json",
      ...(options.body ? { "Content-Type": "application/json", "X-CSRF-Token": csrf } : {}),
    };
    const response = await fetch(path, {
      credentials: "same-origin",
      ...options,
      headers: { ...headers, ...(options.headers || {}) },
    });
    let payload = null;
    try {
      payload = await response.json();
    } catch {
      payload = { status: "error", error: "Invalid JSON response." };
    }
    if (!response.ok || payload.status === "error") {
      throw new Error(payload.error || `Request failed with HTTP ${response.status}`);
    }
    return payload;
  }

  function initKanbanPanel(kanban) {
    const boardSelect = kanban.querySelector("[data-kanban-board-select]");
    const includeDone = kanban.querySelector("[data-kanban-include-done]");
    const statusEl = kanban.querySelector("[data-kanban-status]");
    const metricsEl = kanban.querySelector("[data-kanban-metrics]");
    const detailEl = kanban.querySelector("[data-kanban-detail]");
    const createForm = kanban.querySelector("[data-kanban-create]");
    let board = boardSelect?.value || "default";

    const setKanbanStatus = (text) => {
      if (statusEl) statusEl.textContent = text;
      setStatus(text || "Kanban open");
    };

    const loadBoards = async () => {
      const payload = await kanbanFetch("/agent/kanban/boards");
      const boards = Array.isArray(payload.boards) && payload.boards.length
        ? payload.boards
        : [{ slug: "default", title: "Default" }];
      boardSelect.replaceChildren(
        ...boards.map((item) => {
          const option = document.createElement("option");
          option.value = item.slug || "default";
          option.textContent = item.title || item.slug || "Default";
          return option;
        }),
      );
      if (!boards.some((item) => item.slug === board)) board = boards[0].slug || "default";
      boardSelect.value = board;
    };

    const loadBoard = async () => {
      setKanbanStatus("Loading board");
      const done = includeDone?.checked ? "?include_done=1" : "";
      const payload = await kanbanFetch(`/agent/kanban/board/${encodeURIComponent(board)}${done}`);
      renderMetrics(payload.diagnostics);
      renderBoard(Array.isArray(payload.tasks) ? payload.tasks : []);
      setKanbanStatus("Board loaded");
    };

    const renderMetrics = (diagnostics) => {
      if (!metricsEl) return;
      const items = diagnostics
        ? [
            ["Boards", diagnostics.boards],
            ["Tasks", diagnostics.tasks],
            ["Open", diagnostics.open_tasks],
            ["Blocked", diagnostics.blocked_tasks],
            ["Done", diagnostics.done_tasks],
            ["Claims", diagnostics.running_claims],
          ]
        : [];
      metricsEl.replaceChildren(
        ...items.map(([label, value]) => {
          const span = document.createElement("span");
          span.className = "kanban-metric";
          span.textContent = `${label}: ${value ?? 0}`;
          return span;
        }),
      );
    };

    const renderBoard = (tasks) => {
      const grouped = { todo: [], doing: [], blocked: [], done: [] };
      for (const task of tasks) {
        const status = grouped[task.status] ? task.status : "todo";
        grouped[status].push(task);
      }
      for (const [status, items] of Object.entries(grouped)) {
        const column = kanban.querySelector(`[data-kanban-column="${status}"]`);
        const count = column?.querySelector("h3 b");
        const list = column?.querySelector("div");
        if (count) count.textContent = String(items.length);
        if (!list) continue;
        list.replaceChildren(...items.map((task) => taskCard(task)));
      }
      const navCount = root.querySelector("[data-kanban-nav-count]");
      if (navCount) navCount.textContent = String(tasks.filter((task) => task.status !== "done").length);
    };

    const taskCard = (task) => {
      const card = document.createElement("article");
      card.className = "kanban-card";
      card.dataset.status = task.status || "todo";

      const title = document.createElement("p");
      title.className = "kanban-card-title";
      title.textContent = task.title || task.id;

      const meta = document.createElement("p");
      meta.className = "kanban-card-meta";
      meta.textContent = [task.id, task.assignee ? `@${task.assignee}` : null]
        .filter(Boolean)
        .join(" · ");

      const desc = document.createElement("p");
      desc.className = "kanban-card-desc";
      desc.textContent = task.description || "";

      const actions = document.createElement("div");
      actions.className = "kanban-card-actions";
      actions.append(
        actionButton("Start", () => updateStatus(task.id, "doing")),
        actionButton("Block", () => updateStatus(task.id, "blocked")),
        actionButton("Done", () => updateStatus(task.id, "done")),
        actionButton("Todo", () => updateStatus(task.id, "todo")),
        actionButton("Claim", () => claimTask(task.id)),
        actionButton("Details", () => showTask(task.id)),
      );

      card.append(title, meta);
      if (task.description) card.append(desc);
      card.append(actions);
      return card;
    };

    const actionButton = (label, handler) => {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      button.addEventListener("click", handler);
      return button;
    };

    const updateStatus = async (taskId, nextStatus) => {
      await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}/status`, {
        method: "POST",
        body: JSON.stringify({ status: nextStatus }),
      });
      await loadBoard();
    };

    const claimTask = async (taskId) => {
      await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}/claim`, {
        method: "POST",
        body: "{}",
      });
      await showTask(taskId);
      await loadBoard();
    };

    const assignTask = async (taskId) => {
      const assignee = window.prompt("Assignee");
      if (assignee === null) return;
      await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}/assign`, {
        method: "POST",
        body: JSON.stringify({ assignee }),
      });
      await showTask(taskId);
      await loadBoard();
    };

    const showTask = async (taskId) => {
      const payload = await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}`);
      const task = payload.task || {};
      detailEl.hidden = false;
      detailEl.replaceChildren();

      const title = document.createElement("h3");
      title.textContent = task.title || taskId;
      const meta = document.createElement("p");
      meta.textContent = `${task.id || taskId} · ${task.status || "todo"}${task.assignee ? ` · @${task.assignee}` : ""}`;
      const desc = document.createElement("p");
      desc.textContent = task.description || "No description.";

      const comments = document.createElement("ul");
      for (const comment of payload.comments || []) {
        const item = document.createElement("li");
        item.textContent = `${comment.author || "operator"}: ${comment.body || ""}`;
        comments.append(item);
      }

      const note = document.createElement("textarea");
      note.placeholder = "Comment";
      const actions = document.createElement("div");
      actions.className = "kanban-detail-actions";
      actions.append(
        actionButton("Assign", () => assignTask(taskId)),
        actionButton("Heartbeat", async () => {
          await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}/heartbeat`, {
            method: "POST",
            body: JSON.stringify({ note: "web ui" }),
          });
          await showTask(taskId);
        }),
        actionButton("Comment", async () => {
          const body = note.value.trim();
          if (!body) return;
          await kanbanFetch(`/agent/kanban/tasks/${encodeURIComponent(taskId)}/comments`, {
            method: "POST",
            body: JSON.stringify({ body }),
          });
          note.value = "";
          await showTask(taskId);
        }),
      );
      detailEl.append(title, meta, desc, comments, note, actions);
    };

    createForm?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const formData = new FormData(createForm);
      await kanbanFetch("/agent/kanban/tasks", {
        method: "POST",
        body: JSON.stringify({
          board,
          title: String(formData.get("title") || ""),
          assignee: String(formData.get("assignee") || ""),
          description: String(formData.get("description") || ""),
        }),
      });
      createForm.reset();
      await loadBoards();
      await loadBoard();
    });

    boardSelect?.addEventListener("change", async () => {
      board = boardSelect.value || "default";
      await loadBoard();
    });
    includeDone?.addEventListener("change", loadBoard);
    kanban.querySelector("[data-kanban-refresh]")?.addEventListener("click", loadBoard);
    kanban.querySelector("[data-kanban-reclaim]")?.addEventListener("click", async () => {
      await kanbanFetch("/agent/kanban/reclaim", { method: "POST", body: "{}" });
      await loadBoard();
    });
    kanban.querySelector("[data-kanban-dispatch]")?.addEventListener("click", async () => {
      const payload = await kanbanFetch("/agent/kanban/dispatch", {
        method: "POST",
        body: JSON.stringify({ dry_run: true, max_claims: 3 }),
      });
      const count = payload.dispatch?.candidates?.length ?? payload.dispatch?.claimed?.length ?? 0;
      setKanbanStatus(`Dispatch checked ${count} task${count === 1 ? "" : "s"}`);
    });

    loadBoards()
      .then(loadBoard)
      .catch((error) => setKanbanStatus(error instanceof Error ? error.message : "Kanban failed"));
  }
}
