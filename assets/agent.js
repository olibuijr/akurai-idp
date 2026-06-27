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
  let running = false;
  const toolPanels = new Set([
    "tasks",
    "projects",
    "agy",
    "notes",
    "passvault",
    "cron",
    "kanban",
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

    const csrf = form.querySelector("input[name='_csrf']")?.value || "";
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
      button.addEventListener("click", () => openPanel(button.dataset.panelTrigger));
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
    button.addEventListener("click", () => openPanel(button.dataset.panelTrigger));
  });

  root.querySelectorAll("[data-agent-prompt]").forEach((button) => {
    button.addEventListener("click", () => fillPrompt(button.dataset.agentPrompt));
  });

  form?.addEventListener("submit", submitStream);
}
