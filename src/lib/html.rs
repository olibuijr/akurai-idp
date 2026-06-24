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
// Shared design tokens, reset, and component styles
// ---------------------------------------------------------------------------
const BASE_STYLES: &str = r#"
  /* ── Reset ── */
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

  /* ── Tokens ── */
  :root {
    --bg:           #F8FAFC;
    --surface:      #FFFFFF;
    --primary:      #0F172A;
    --secondary:    #1E3A8A;
    --accent:       #A16207;
    --muted:        #E8ECF1;
    --border:       #CBD5E1;
    --border-light: #E2E8F0;
    --ink:          #0F172A;
    --ink-muted:    #475569;
    --ink-faint:    #94A3B8;
    --error-bg:     #FEF2F2;
    --error-border: #FECACA;
    --error-ink:    #991B1B;
    --success-bg:   #F0FDF4;
    --success-border:#BBF7D0;
    --success-ink:  #14532D;
    --info-bg:      #EFF6FF;
    --info-border:  #BFDBFE;
    --info-ink:     #1E40AF;
    --warn-bg:      #FFFBEB;
    --warn-border:  #FDE68A;
    --warn-ink:     #78350F;
    --radius-sm:    6px;
    --radius:       10px;
    --radius-lg:    14px;
    --shadow:       0 1px 3px rgba(15,23,42,.06), 0 8px 32px rgba(15,23,42,.08);
    --transition:   150ms cubic-bezier(.4,0,.2,1);
  }

  /* ── Base ── */
  body {
    min-height: 100dvh;
    background: var(--bg);
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
  input[type=text] {
    display: block;
    width: 100%;
    padding: 0.625rem 0.875rem;
    border: 1.5px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    font-family: inherit;
    font-size: 0.9375rem;
    color: var(--ink);
    outline: none;
    transition: border-color var(--transition), box-shadow var(--transition);
  }
  input::placeholder { color: var(--ink-faint); }
  input:hover { border-color: #94A3B8; }
  input:focus {
    border-color: var(--secondary);
    box-shadow: 0 0 0 3px rgba(30,58,138,.12);
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

  .btn-primary { background: var(--primary); color: #fff; }
  .btn-primary:hover { background: var(--secondary); box-shadow: 0 4px 14px rgba(30,58,138,.25); }

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
    background: var(--primary);
    color: #fff;
  }
  button[type=submit]:hover { background: var(--secondary); box-shadow: 0 4px 14px rgba(30,58,138,.2); }
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
"#;

const WORDMARK: &str = r##"
  <a href="/account" class="wordmark" aria-label="AkurAI ID home">
    <svg class="wordmark-icon" viewBox="0 0 34 34" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="34" height="34" rx="8" fill="#0F172A"/>
      <path d="M17 8L25 24H9L17 8Z" fill="#FFFFFF" opacity="0.95"/>
      <path d="M17 14L21 24H13L17 14Z" fill="#0F172A"/>
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
pub fn auth_page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{title} — AkurAI ID</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&display=swap" rel="stylesheet" />
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
      padding: 2.5rem 2.25rem;
      width: 100%;
      max-width: 420px;
    }}
    .trust-footer {{ margin-top: 2.5rem; }}
    @media (max-width: 480px) {{ .card {{ padding: 2rem 1.5rem; }} }}
  </style>
</head>
<body>
<div class="card">
  {WORDMARK}
  {body}
</div>
{TRUST_FOOTER}
</body>
</html>"#,
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
    .badge-green { background: #DCFCE7; color: #14532D; }
    .badge-gray  { background: var(--muted); color: var(--ink-muted); }
    .badge-blue  { background: #DBEAFE; color: #1E40AF; }

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
pub fn account_page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{title} — AkurAI ID</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&display=swap" rel="stylesheet" />
  <style>
    {BASE_STYLES}
    {ACCOUNT_EXTRA_STYLES}
  </style>
</head>
<body>
<div class="card">
  {WORDMARK}
  {body}
</div>
{TRUST_FOOTER}
</body>
</html>"#,
        title = esc_html(title),
        BASE_STYLES = BASE_STYLES,
        ACCOUNT_EXTRA_STYLES = ACCOUNT_EXTRA_STYLES,
        WORDMARK = WORDMARK,
        body = body,
        TRUST_FOOTER = TRUST_FOOTER,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_html_works() {
        assert_eq!(esc_html("<b>\"hi\" & 'bye'</b>"), "&lt;b&gt;&quot;hi&quot; &amp; &#x27;bye&#x27;&lt;/b&gt;");
    }

    #[test]
    fn auth_page_contains_card() {
        let html = auth_page("Login", "<h1>Login</h1>");
        assert!(html.contains("max-width: 420px"));
        assert!(html.contains("AkurAI"));
        assert!(html.contains("<h1>Login</h1>"));
    }

    #[test]
    fn account_page_contains_wider_card() {
        let html = account_page("Settings", "<h1>Settings</h1>");
        assert!(html.contains("max-width: 580px"));
        assert!(html.contains("account-nav"));
    }
}
