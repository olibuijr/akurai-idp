/// HTML escape: replaces &, <, >, ", ' with their HTML entities.
pub fn esc_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Locale — resolved from the akurai-lang cookie (default: Icelandic)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Locale {
    Is,
    En,
}

impl Locale {
    /// Parse locale from a raw `Cookie:` header value.
    /// `akurai-lang=en` → English; anything else / absent → Icelandic (default).
    pub fn from_cookie_header(cookie_header: Option<&str>) -> Self {
        if let Some(cookies) = cookie_header {
            for part in cookies.split(';') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("akurai-lang=") {
                    if val.trim() == "en" {
                        return Locale::En;
                    }
                }
            }
        }
        Locale::Is
    }

    /// BCP-47 language tag for `<html lang="...">`.
    pub fn lang_attr(&self) -> &'static str {
        match self {
            Locale::Is => "is",
            Locale::En => "en",
        }
    }
}

/// Pick between Icelandic and English strings.
/// Usage: `t(locale, "Skrá inn", "Sign in")`
pub fn t(locale: Locale, is: &'static str, en: &'static str) -> &'static str {
    match locale {
        Locale::Is => is,
        Locale::En => en,
    }
}

// ---------------------------------------------------------------------------
// Shared design tokens, reset, and component styles
// ---------------------------------------------------------------------------
const BASE_STYLES: &str = r#"
  /* ── Reset ── */
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

  /* ── Tokens ── */
  :root {
    --bg:           #080A0D;
    --surface:      rgba(18, 23, 31, .76);
    --surface-2:    rgba(28, 36, 48, .7);
    --primary:      #F8FAFC;
    --secondary:    #67E8F9;
    --accent:       #B7F36B;
    --accent-2:     #F7C948;
    --muted:        rgba(148, 163, 184, .14);
    --border:       rgba(203, 213, 225, .22);
    --border-light: rgba(203, 213, 225, .12);
    --ink:          #EEF6FF;
    --ink-muted:    #A9B8C8;
    --ink-faint:    #748398;
    --error-bg:     rgba(127, 29, 29, .26);
    --error-border: rgba(248, 113, 113, .34);
    --error-ink:    #FCA5A5;
    --success-bg:   rgba(20, 83, 45, .26);
    --success-border:rgba(134, 239, 172, .34);
    --success-ink:  #BBF7D0;
    --info-bg:      rgba(8, 145, 178, .22);
    --info-border:  rgba(103, 232, 249, .34);
    --info-ink:     #A5F3FC;
    --warn-bg:      rgba(120, 53, 15, .24);
    --warn-border:  rgba(251, 191, 36, .34);
    --warn-ink:     #FDE68A;
    --radius-sm:    6px;
    --radius:       12px;
    --radius-lg:    18px;
    --shadow:       0 18px 50px rgba(0,0,0,.38), inset 0 1px 0 rgba(255,255,255,.08);
    --shadow-soft:  10px 10px 28px rgba(0,0,0,.35), -8px -8px 22px rgba(255,255,255,.035);
    --transition:   160ms cubic-bezier(.4,0,.2,1);
  }

  /* ── Base ── */
  body {
    min-height: 100dvh;
    background:
      linear-gradient(rgba(255,255,255,.035) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255,255,255,.035) 1px, transparent 1px),
      var(--bg);
    background-size: 44px 44px, 44px 44px, auto;
    font-family: "DM Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: var(--ink);
    -webkit-font-smoothing: antialiased;
  }

  /* ── Wordmark ── */
  .wordmark {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    margin-bottom: 2rem;
    text-decoration: none;
  }
  .wordmark-icon { width: 34px; height: 34px; flex-shrink: 0; }
  .wordmark-name {
    font-size: 1.125rem;
    font-weight: 700;
    color: var(--primary);
    letter-spacing: -0.02em;
    line-height: 1;
  }
  .wordmark-badge {
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
    margin-left: 0.25rem;
    vertical-align: super;
  }

  /* ── Page heading ── */
  .page-heading {
    font-size: 1.375rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--primary);
    margin-bottom: 0.375rem;
  }
  .page-sub {
    font-size: 0.875rem;
    color: var(--ink-muted);
    line-height: 1.5;
    margin-bottom: 1.75rem;
  }

  /* ── Section heading (h3 inside account pages) ── */
  .section-heading {
    font-size: 1rem;
    font-weight: 600;
    color: var(--primary);
    margin-top: 1.75rem;
    margin-bottom: 0.75rem;
  }

  /* ── Form fields ── */
  .field { margin-bottom: 1.125rem; }
  label {
    display: block;
    font-size: 0.8125rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--primary);
    margin-bottom: 0.4rem;
  }
  input[type=email],
  input[type=password],
  input[type=text],
  textarea {
    display: block;
    width: 100%;
    padding: 0.625rem 0.875rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgba(5, 8, 12, .68);
    font-family: inherit;
    font-size: 0.9375rem;
    color: var(--ink);
    outline: none;
    box-shadow: inset 0 1px 1px rgba(255,255,255,.04), inset 0 12px 24px rgba(0,0,0,.18);
    transition: border-color var(--transition), box-shadow var(--transition), background var(--transition);
  }
  textarea {
    min-height: 180px;
    resize: vertical;
    font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    line-height: 1.55;
  }
  input::placeholder { color: var(--ink-faint); }
  textarea::placeholder { color: var(--ink-faint); }
  input:hover, textarea:hover { border-color: rgba(103, 232, 249, .4); }
  input:focus {
    border-color: var(--secondary);
    box-shadow: 0 0 0 3px rgba(103, 232, 249, .13), inset 0 12px 24px rgba(0,0,0,.18);
  }
  textarea:focus {
    border-color: var(--secondary);
    box-shadow: 0 0 0 3px rgba(103, 232, 249, .13), inset 0 12px 24px rgba(0,0,0,.18);
  }
  input[readonly]:focus { box-shadow: none; border-color: var(--border); }

  /* ── Buttons ── */
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    padding: 0.65rem 1.125rem;
    border: none;
    border-radius: var(--radius-sm);
    font-family: inherit;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    cursor: pointer;
    text-decoration: none;
    transition: background var(--transition), box-shadow var(--transition), transform var(--transition);
  }
  .btn:active { transform: translateY(1px); box-shadow: none !important; }
  .btn:focus-visible { outline: 2px solid var(--secondary); outline-offset: 2px; }

  .btn-primary { background: linear-gradient(135deg, var(--secondary), var(--accent)); color: #071013; }
  .btn-primary:hover { box-shadow: 0 10px 30px rgba(103, 232, 249, .2); }

  .btn-danger  { background: #991B1B; color: #fff; }
  .btn-danger:hover  { background: #7F1D1D; box-shadow: 0 4px 14px rgba(127,29,29,.25); }

  .btn-ghost   { background: transparent; color: var(--secondary); border: 1.5px solid var(--border); }
  .btn-ghost:hover   { background: var(--muted); }

  .btn-full    { width: 100%; }
  .btn-sm      { padding: 0.4rem 0.75rem; font-size: 0.8125rem; }

  /* Legacy submit shim */
  button[type=submit] {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.65rem 1.125rem;
    border: none;
    border-radius: var(--radius-sm);
    font-family: inherit;
    font-size: 0.9375rem;
    font-weight: 600;
    cursor: pointer;
    transition: background var(--transition), box-shadow var(--transition), transform var(--transition);
    background: linear-gradient(135deg, var(--secondary), var(--accent));
    color: #071013;
  }
  button[type=submit]:hover { box-shadow: 0 10px 30px rgba(103, 232, 249, .2); }
  button[type=submit]:active { transform: translateY(1px); box-shadow: none; }
  button[type=submit]:focus-visible { outline: 2px solid var(--secondary); outline-offset: 2px; }
  button[type=submit].danger  { background: #991B1B; }
  button[type=submit].danger:hover  { background: #7F1D1D; }
  button[type=submit].secondary { background: var(--ink-muted); }
  button[type=submit].secondary:hover { background: #334155; }

  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { transition: none !important; }
  }

  /* ── Alerts ── */
  .alert {
    display: flex;
    align-items: flex-start;
    gap: 0.625rem;
    border-radius: var(--radius-sm);
    padding: 0.75rem 0.875rem;
    font-size: 0.875rem;
    line-height: 1.5;
    margin-bottom: 1.25rem;
  }
  .alert-icon { flex-shrink: 0; margin-top: 1px; }
  .alert--error   { background: var(--error-bg);   border: 1px solid var(--error-border);   color: var(--error-ink); }
  .alert--success { background: var(--success-bg); border: 1px solid var(--success-border); color: var(--success-ink); }
  .alert--info    { background: var(--info-bg);    border: 1px solid var(--info-border);    color: var(--info-ink); }
  .alert--warn    { background: var(--warn-bg);    border: 1px solid var(--warn-border);    color: var(--warn-ink); }

  .error, .success {
    border-radius: var(--radius-sm);
    padding: .75rem .875rem;
    margin: .75rem 0 1rem;
    line-height: 1.5;
    border: 1px solid;
  }
  .error { color: var(--error-ink); background: var(--error-bg); border-color: var(--error-border); }
  .success { color: var(--success-ink); background: var(--success-bg); border-color: var(--success-border); }

  p { color: var(--ink-muted); line-height: 1.6; margin: .75rem 0; }
  h2 { color: var(--primary); font-size: 1.25rem; margin: 0 0 .75rem; }
  h3 { color: var(--primary); font-size: 1rem; margin: 1.25rem 0 .5rem; }
  dl { display: grid; gap: .7rem; margin: 1rem 0 1.25rem; }
  dt { color: var(--ink-faint); font-size: .72rem; text-transform: uppercase; letter-spacing: .08em; }
  dd { color: var(--ink); font-weight: 600; margin-top: .15rem; }
  nav ul { display: grid; gap: .5rem; list-style: none; margin-top: 1rem; }
  nav a, .action-link { color: var(--secondary); text-decoration: none; font-weight: 600; }
  nav a:hover, .action-link:hover { color: var(--accent); }
  .small { font-size: .8125rem; }
  .mono { font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace; }
  .danger { color: var(--error-ink); }

  /* ── Divider ── */
  .divider { height: 1px; background: var(--border-light); margin: 1.5rem 0; }

  /* ── Links ── */
  .page-footer {
    font-size: 0.8125rem;
    color: var(--ink-muted);
    margin-top: 1.5rem;
    line-height: 1.6;
  }
  .page-footer a { color: var(--secondary); text-decoration: none; font-weight: 500; }
  .page-footer a:hover { text-decoration: underline; }
  .page-footer a:focus-visible { outline: 2px solid var(--secondary); outline-offset: 2px; border-radius: 2px; }

  /* ── Trust footer ── */
  .trust-footer {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: var(--ink-faint);
    font-size: 0.75rem;
  }
  .trust-footer svg { flex-shrink: 0; }

  /* ── Page top (wordmark row + lang toggle) ── */
  .page-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2rem;
  }
  .page-top .wordmark { margin-bottom: 0; }

  /* ── Lang toggle ── */
  .lang-toggle {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .lang-opt {
    appearance: none;
    border: 0;
    background: transparent;
    color: var(--ink-faint);
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: .02em;
    padding: 5px 9px;
    cursor: pointer;
    line-height: 1;
    transition: background-color .15s ease, color .15s ease;
  }
  .lang-opt + .lang-opt { border-left: 1px solid var(--border); }
  .lang-opt:hover { color: var(--ink); }
  .lang-opt.is-active { background: var(--accent); color: #071013; }
  .lang-opt:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
"#;

const WORDMARK: &str = r##"
  <a href="/account" class="wordmark" aria-label="AkurAI ID home">
    <svg class="wordmark-icon" viewBox="0 0 34 34" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <linearGradient id="wm-g" x1="0" y1="0" x2="34" y2="34" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stop-color="#67E8F9"/>
          <stop offset="100%" stop-color="#B7F36B"/>
        </linearGradient>
      </defs>
      <rect width="34" height="34" rx="8" fill="url(#wm-g)"/>
      <g transform="translate(1,1)">
        <path fill-rule="evenodd" clip-rule="evenodd" d="M13.38 3.23c1.05-2.23 4.19-2.23 5.24 0l11.05 23.62A2.91 2.91 0 0 1 27.03 31h-3.42c-1.17 0-2.22-.7-2.67-1.78l-2.05-4.93h-5.78l-2.05 4.93A2.9 2.9 0 0 1 8.39 31H4.97a2.91 2.91 0 0 1-2.64-4.15L13.38 3.23ZM16 11.17l-5.13 13.1h10.26L16 11.17Z" fill="#071013"/>
        <path d="M10.38 20.54h11.24a2.23 2.23 0 0 1 0 4.46H10.38a2.23 2.23 0 0 1 0-4.46Z" fill="#071013"/>
      </g>
    </svg>
    <span class="wordmark-name">AkurAI<span class="wordmark-badge">ID</span></span>
  </a>"##;

const TRUST_FOOTER: &str = r#"
<div class="trust-footer">
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
  Secured by AkurAI ID &mdash; OIDC / OAuth 2.0
</div>"#;

// ---------------------------------------------------------------------------
// authPage -- centered single-column card (login, MFA, etc.)
// ---------------------------------------------------------------------------

/// Full HTML page with a centered 420px card for auth flows (login, MFA).
/// Includes DM Sans font, navy/gold palette, WCAG AAA contrast, and AkurAI ID wordmark.
pub fn auth_page(locale: Locale, title: &str, body: &str) -> String {
    let lang = locale.lang_attr();
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <script>(function(){{try{{var m=document.cookie.match(/(?:^|; )akurai-theme=([^;]*)/);var t=(m?decodeURIComponent(m[1]):null)||localStorage.getItem("akurai-theme");if(!t)t=matchMedia("(prefers-color-scheme: light)").matches?"akurai-light":"akurai";document.documentElement.setAttribute("data-theme",t);}}catch(e){{}}}})()</script>
  <title>{title} — AkurAI ID</title>
  <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet" />
  <link rel="stylesheet" href="/themes.css" />
  <style>
    {BASE_STYLES}
    body {{
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 1.5rem 1rem;
    }}
    .card {{
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      box-shadow: var(--shadow);
      backdrop-filter: blur(22px) saturate(130%);
      padding: 2.5rem 2.25rem;
      width: 100%;
      max-width: 420px;
    }}
    .trust-footer {{ margin-top: 2.5rem; }}
    @media (max-width: 480px) {{ .card {{ padding: 2rem 1.5rem; }} }}
  </style>
  <script type="module" src="/theme.js"></script>
  <script type="module" src="/lang.js"></script>
</head>
<body>
<div class="card">
  <div class="page-top">
    {WORDMARK}
    <span data-lang-toggle></span>
  </div>
  {body}
</div>
{TRUST_FOOTER}
</body>
</html>"#,
        lang = lang,
        title = esc_html(title),
        BASE_STYLES = BASE_STYLES,
        WORDMARK = WORDMARK,
        body = body,
        TRUST_FOOTER = TRUST_FOOTER,
    )
}

// ---------------------------------------------------------------------------
// accountPage -- top-aligned wider card (account settings pages)
// ---------------------------------------------------------------------------

/// Account-page-specific extra CSS (nav, tables, badges, meta rows, etc.)
const ACCOUNT_EXTRA_STYLES: &str = r#"
    body {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 2.5rem 1rem 4rem;
    }
    .card {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      box-shadow: var(--shadow);
      backdrop-filter: blur(22px) saturate(130%);
      padding: 2.5rem 2.25rem;
      width: 100%;
      max-width: 580px;
    }

    /* ── Account nav ── */
    .account-nav {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      margin-bottom: 1.75rem;
    }
    .account-nav a {
      display: inline-block;
      padding: 0.375rem 0.875rem;
      border-radius: 999px;
      font-size: 0.8125rem;
      font-weight: 500;
      color: var(--ink-muted);
      text-decoration: none;
      border: 1.5px solid transparent;
      transition: color var(--transition), border-color var(--transition), background var(--transition);
    }
    .account-nav a:hover  { color: var(--primary); background: var(--muted); }
    .account-nav a.active { color: var(--secondary); border-color: var(--border); background: var(--surface); font-weight: 600; }
    .account-nav a:focus-visible { outline: 2px solid var(--secondary); outline-offset: 2px; }

    /* ── Meta row (profile display) ── */
    .meta-row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 0.75rem 0;
      border-bottom: 1px solid var(--border-light);
      font-size: 0.9rem;
    }
    .meta-row:last-child { border-bottom: none; }
    .meta-label {
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      color: var(--ink-faint);
    }
    .meta-value { font-weight: 500; color: var(--ink); }

    /* ── Badges ── */
    .badge {
      display: inline-block;
      padding: 0.2rem 0.6rem;
      border-radius: 999px;
      font-size: 0.75rem;
      font-weight: 600;
    }
    .badge-green, .badge-ok { background: var(--success-bg); color: var(--success-ink); border: 1px solid var(--success-border); }
    .badge-gray  { background: var(--muted); color: var(--ink-muted); }
    .badge-blue  { background: #DBEAFE; color: #1E40AF; }
    .badge-warn  { background: var(--warn-bg); color: var(--warn-ink); border: 1px solid var(--warn-border); }

    /* ── Table ── */
    .table-wrap { overflow-x: auto; margin-top: 0.5rem; }
    table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
    th {
      text-align: left;
      padding: 0.5rem 0.625rem;
      color: var(--ink-faint);
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      border-bottom: 1.5px solid var(--border);
    }
    td { padding: 0.625rem 0.625rem; border-bottom: 1px solid var(--border-light); vertical-align: middle; }
    tr.current-session td { background: var(--info-bg); }
    tr:last-child td { border-bottom: none; }
    .revoke-btn {
      background: none;
      border: none;
      cursor: pointer;
      color: var(--error-ink);
      font-size: 0.8125rem;
      font-weight: 500;
      padding: 0;
      font-family: inherit;
    }
    .revoke-btn:hover { text-decoration: underline; }
    .revoke-btn:focus-visible { outline: 2px solid var(--error-ink); outline-offset: 2px; border-radius: 2px; }

    /* ── Code / monospace ── */
    code {
      font-family: ui-monospace, "Cascadia Code", monospace;
      background: var(--muted);
      padding: 0.15rem 0.4rem;
      border-radius: 4px;
      font-size: 0.85rem;
    }

    /* ── Secret block ── */
    .secret-block {
      background: var(--bg);
      border: 1.5px solid var(--border);
      border-radius: var(--radius);
      padding: 1rem 1.125rem;
      margin-bottom: 1rem;
    }
    .secret-block p { font-size: 0.8125rem; color: var(--ink-muted); margin-bottom: 0.5rem; }
    .secret-block code { display: block; word-break: break-all; background: transparent; padding: 0; font-size: 0.875rem; }

    /* ── Backup codes grid ── */
    .backup-codes {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 0.375rem;
      margin: 0.75rem 0 1rem;
    }
    .backup-codes code {
      display: block;
      padding: 0.4rem 0.5rem;
      background: var(--muted);
      border-radius: var(--radius-sm);
      text-align: center;
    }

    /* ── Form inline (for compact single-field forms) ── */
    .form-inline { display: flex; gap: 0.5rem; align-items: flex-end; }
    .form-inline button { flex-shrink: 0; }

    /* ── Action links ── */
    .action-link { color: var(--secondary); text-decoration: none; font-size: 0.875rem; font-weight: 500; }
    .action-link:hover { text-decoration: underline; }
    .action-link.danger { color: var(--error-ink); }

    /* ── Bottom trust bar ── */
    .trust-footer { margin-top: 2.5rem; }

    @media (max-width: 640px) {
      .card { padding: 2rem 1.25rem; }
      .account-nav { gap: 0.125rem; }
      th, td { padding: 0.5rem 0.375rem; }
    }
"#;

/// Full HTML page with a top-aligned 580px card for account settings pages.
/// Includes nav, tables, badges CSS on top of the base design system.
pub fn account_page(locale: Locale, title: &str, body: &str) -> String {
    let lang = locale.lang_attr();
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <script>(function(){{try{{var m=document.cookie.match(/(?:^|; )akurai-theme=([^;]*)/);var t=(m?decodeURIComponent(m[1]):null)||localStorage.getItem("akurai-theme");if(!t)t=matchMedia("(prefers-color-scheme: light)").matches?"akurai-light":"akurai";document.documentElement.setAttribute("data-theme",t);}}catch(e){{}}}})()</script>
  <title>{title} — AkurAI ID</title>
  <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet" />
  <link rel="stylesheet" href="/themes.css" />
  <style>
    {BASE_STYLES}
    {ACCOUNT_EXTRA_STYLES}
  </style>
  <script type="module" src="/theme.js"></script>
  <script type="module" src="/lang.js"></script>
</head>
<body>
<div class="card">
  <div class="page-top">
    {WORDMARK}
    <span data-lang-toggle></span>
  </div>
  {body}
</div>
{TRUST_FOOTER}
</body>
</html>"#,
        lang = lang,
        title = esc_html(title),
        BASE_STYLES = BASE_STYLES,
        ACCOUNT_EXTRA_STYLES = ACCOUNT_EXTRA_STYLES,
        WORDMARK = WORDMARK,
        body = body,
        TRUST_FOOTER = TRUST_FOOTER,
    )
}

const CONSOLE_EXTRA_STYLES: &str = r#"
    body {
      min-height: 100dvh;
      padding: 2rem;
    }
    .console-wrap {
      width: min(1120px, 100%);
      margin: 0 auto;
    }
    .console-topbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
      margin-bottom: 1rem;
    }
    .console-topbar .wordmark { margin-bottom: 0; }
    .console-status {
      display: flex;
      gap: .5rem;
      flex-wrap: wrap;
      justify-content: flex-end;
    }
    .console-dot {
      display: inline-flex;
      align-items: center;
      gap: .45rem;
      padding: .35rem .65rem;
      border-radius: 999px;
      color: var(--ink-muted);
      border: 1px solid var(--border);
      background: rgba(255,255,255,.045);
      font-size: .78rem;
      font-family: ui-monospace, "JetBrains Mono", "SFMono-Regular", Menlo, Consolas, monospace;
    }
    .console-dot::before {
      content: "";
      width: .45rem;
      height: .45rem;
      border-radius: 999px;
      background: var(--accent);
      box-shadow: 0 0 16px rgba(183, 243, 107, .55);
    }
    .console-card {
      border: 1px solid var(--border);
      border-radius: 24px;
      background: linear-gradient(145deg, rgba(18,23,31,.84), rgba(9,12,17,.72));
      box-shadow: var(--shadow), var(--shadow-soft);
      backdrop-filter: blur(24px) saturate(145%);
      overflow: hidden;
      position: relative;
    }
    .console-card::before {
      content: "";
      position: absolute;
      inset: 0;
      pointer-events: none;
      background:
        linear-gradient(120deg, rgba(103,232,249,.16), transparent 32%),
        linear-gradient(300deg, rgba(183,243,107,.12), transparent 38%);
      opacity: .75;
    }
    .console-card > * { position: relative; }
    @media (max-width: 860px) {
      body { padding: 1rem; }
      .console-topbar { align-items: flex-start; flex-direction: column; }
      .console-status { justify-content: flex-start; }
    }
"#;

pub fn console_page(title: &str, body: &str) -> String {
    console_page_with_styles(title, body, "")
}

pub fn console_page_with_styles(title: &str, body: &str, extra_styles: &str) -> String {
    console_page_with_theme(title, body, extra_styles, None)
}

pub fn console_page_with_theme(
    title: &str,
    body: &str,
    extra_styles: &str,
    theme: Option<&str>,
) -> String {
    let theme_attr = theme
        .filter(|theme| !theme.is_empty())
        .map(|theme| format!(r#" data-theme="{}""#, esc_html(theme)))
        .unwrap_or_default();
    let theme_default = theme
        .filter(|theme| {
            theme
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
        .unwrap_or("");
    format!(
        r#"<!DOCTYPE html>
<html lang="en"{theme_attr}>
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <script>(function(){{try{{var d="{theme_default}";var m=document.cookie.match(/(?:^|; )akurai-theme=([^;]*)/);var t=(m?decodeURIComponent(m[1]):null)||d||localStorage.getItem("akurai-theme");if(!t)t=matchMedia("(prefers-color-scheme: light)").matches?"akurai-light":"akurai";document.documentElement.setAttribute("data-theme",t);}}catch(e){{}}}})()</script>
  <title>{title} — AkurAI ID</title>
  <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet" />
  <style>
    {BASE_STYLES}
    {CONSOLE_EXTRA_STYLES}
    {extra_styles}
  </style>
  <link rel="stylesheet" href="/themes.css" />
  <script type="module" src="/theme.js"></script>
</head>
<body>
<main class="console-wrap">
  <header class="console-topbar">
    {WORDMARK}
    <div class="console-status">
      <span class="console-dot">agent.olibuijr.com</span>
      <span class="console-dot">AkurAI-RustAgent</span>
    </div>
  </header>
  <div class="console-card">
    {body}
  </div>
</main>
</body>
</html>"#,
        title = esc_html(title),
        BASE_STYLES = BASE_STYLES,
        CONSOLE_EXTRA_STYLES = CONSOLE_EXTRA_STYLES,
        extra_styles = extra_styles,
        theme_attr = theme_attr,
        theme_default = theme_default,
        WORDMARK = WORDMARK,
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_html_works() {
        assert_eq!(
            esc_html("<b>\"hi\" & 'bye'</b>"),
            "&lt;b&gt;&quot;hi&quot; &amp; &#x27;bye&#x27;&lt;/b&gt;"
        );
    }

    #[test]
    fn auth_page_contains_card() {
        let html = auth_page(Locale::Is, "Login", "<h1>Login</h1>");
        assert!(html.contains("max-width: 420px"));
        assert!(html.contains("AkurAI"));
        assert!(html.contains("<h1>Login</h1>"));
    }

    #[test]
    fn auth_page_locale_is() {
        let html = auth_page(Locale::Is, "Skrá inn", "");
        assert!(html.contains(r#"<html lang="is""#));
    }

    #[test]
    fn auth_page_locale_en() {
        let html = auth_page(Locale::En, "Sign in", "");
        assert!(html.contains(r#"<html lang="en""#));
    }

    #[test]
    fn locale_from_cookie_header() {
        assert_eq!(Locale::from_cookie_header(None).lang_attr(), "is");
        assert_eq!(
            Locale::from_cookie_header(Some("foo=bar")).lang_attr(),
            "is"
        );
        assert_eq!(
            Locale::from_cookie_header(Some("akurai-lang=en")).lang_attr(),
            "en"
        );
        assert_eq!(
            Locale::from_cookie_header(Some("x=1; akurai-lang=en; y=2")).lang_attr(),
            "en"
        );
    }

    #[test]
    fn account_page_contains_wider_card() {
        let html = account_page(Locale::Is, "Settings", "<h1>Settings</h1>");
        assert!(html.contains("max-width: 580px"));
        assert!(html.contains("account-nav"));
    }

    #[test]
    fn console_page_contains_agent_shell() {
        let html = console_page("Agent", "<section>Agent</section>");
        assert!(html.contains("console-card"));
        assert!(html.contains("AkurAI-RustAgent"));
    }
}
