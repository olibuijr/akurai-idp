const roots = document.querySelectorAll("[data-agent-ui]");

for (const root of roots) {
  const panel = root.querySelector("[data-agent-panel]");
  const prompt = root.querySelector("textarea[name='prompt']");
  const status = root.querySelector("[data-agent-status]");
  const session = root.dataset.session || "default";
  const notesKey = `akurai-agent-notes:${session}`;

  const setStatus = (text) => {
    if (status) status.textContent = text;
  };

  const activate = (key) => {
    root.querySelectorAll("[data-panel-trigger]").forEach((button) => {
      button.classList.toggle("active", button.dataset.panelTrigger === key);
      button.classList.toggle("agent-mode-active", button.dataset.panelTrigger === key);
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
}
