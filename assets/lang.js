// AkurAI language toggle — native ESM, zero deps.
//
// Locale is resolved SERVER-SIDE per request from the `akurai-lang` cookie (see
// the framework's i18n wiring); with no cookie the default locale (Icelandic) is
// served. This toggle just writes that cookie — scoped to the parent domain so
// the choice is suite-wide, like the theme — and reloads so the server re-renders
// in the chosen language.

const COOKIE = "akurai-lang";
const DEFAULT_LANG = "is";
// Display labels for the two shipped locales.
const LANGS = [
  { code: "is", label: "IS", title: "Íslenska" },
  { code: "en", label: "EN", title: "English" },
];
const ONE_YEAR = 31536000;

function cookieDomain() {
  const h = location.hostname;
  if (!h || h === "localhost" || /^[0-9.]+$/.test(h)) return "";
  const parts = h.split(".");
  if (parts.length < 2) return "";
  return "." + parts.slice(-2).join(".");
}

function currentLang() {
  const m = document.cookie.match(/(?:^|; )akurai-lang=([^;]*)/);
  const v = m ? decodeURIComponent(m[1]) : null;
  // Mirror the server: cookie value if a known locale, else default (Icelandic).
  return LANGS.some((l) => l.code === v) ? v : DEFAULT_LANG;
}

function setLang(code) {
  if (code === currentLang()) return;
  const dom = cookieDomain();
  const secure = location.protocol === "https:" ? "; secure" : "";
  document.cookie =
    `${COOKIE}=${encodeURIComponent(code)}; path=/; max-age=${ONE_YEAR}; samesite=lax` +
    (dom ? `; domain=${dom}` : "") +
    secure;
  location.reload();
}

// Build a small segmented IS/EN control into `mount`.
export function mountLangToggle(mount) {
  if (!mount) return;
  const active = currentLang();
  const group = document.createElement("div");
  group.className = "lang-toggle";
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", "Language");
  for (const l of LANGS) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "lang-opt" + (l.code === active ? " is-active" : "");
    btn.textContent = l.label;
    btn.title = l.title;
    btn.setAttribute("aria-pressed", l.code === active ? "true" : "false");
    btn.addEventListener("click", () => setLang(l.code));
    group.appendChild(btn);
  }
  mount.appendChild(group);
}

// Auto-mount into a `[data-lang-toggle]` slot.
if (typeof document !== "undefined") {
  const boot = () => {
    const slot = document.querySelector("[data-lang-toggle]");
    if (slot) mountLangToggle(slot);
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
}
