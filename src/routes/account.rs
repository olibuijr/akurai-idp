use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::with_db;
use crate::lib::audit::{
    log_audit_event, SESSION_REVOKED, USER_MFA_DISABLED, USER_MFA_ENABLED,
    USER_PASSWORD_CHANGED,
};
use crate::lib::crypto::{generate_secure_token, sha256};
use crate::lib::html::{account_page, esc_html};
use crate::lib::password::{hash_password, verify_password};
use crate::lib::totp::{generate_backup_codes, generate_totp_secret, verify_totp};

pub fn router() -> Router {
    Router::new()
        .route("/account", get(account_overview))
        .route("/account/password", get(password_page).post(password_submit))
        .route("/account/mfa", get(mfa_status))
        .route("/account/mfa/setup", get(mfa_setup_page).post(mfa_setup_submit))
        .route("/account/mfa/disable", post(mfa_disable))
        .route("/account/sessions", get(sessions_page))
        .route("/account/sessions/{id}/revoke", post(session_revoke))
        .route("/account/sessions/revoke-all", post(sessions_revoke_all))
}

// ---------------------------------------------------------------------------
// Form structs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PasswordForm {
    current_password: Option<String>,
    new_password: Option<String>,
    _csrf: Option<String>,
}

#[derive(Deserialize)]
struct MfaSetupForm {
    code: Option<String>,
    secret: Option<String>,
    _csrf: Option<String>,
}

#[derive(Deserialize)]
struct MfaDisableForm {
    password: Option<String>,
    _csrf: Option<String>,
}

// ---------------------------------------------------------------------------
// Session helper (inline auth check)
// ---------------------------------------------------------------------------

struct SessionUser {
    session_id: String,
    user_id: String,
    tenant_id: String,
    email: String,
}

fn require_session(headers: &HeaderMap) -> Result<SessionUser, Response> {
    let session_id = match get_cookie(headers, "idp_session") {
        Some(s) => s,
        None => return Err(redirect("/login")),
    };

    let now = now_epoch();

    let result = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.user_id, u.tenant_id, u.email
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.id = ?1 AND s.expires_at > ?2",
            )
            .ok()?;
        stmt.query_row(rusqlite::params![session_id, now], |row| {
            Ok(SessionUser {
                session_id: row.get(0)?,
                user_id: row.get(1)?,
                tenant_id: row.get(2)?,
                email: row.get(3)?,
            })
        })
        .ok()
    });

    match result {
        Some(u) => Ok(u),
        None => Err(redirect("/login")),
    }
}

// ---------------------------------------------------------------------------
// GET /account
// ---------------------------------------------------------------------------

async fn account_overview(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let (mfa_enabled, session_count, tenant_name) = with_db(|conn| {
        let mfa: bool = conn
            .query_row(
                "SELECT mfa_enabled FROM users WHERE id = ?1",
                rusqlite::params![user.user_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v != 0)
            .unwrap_or(false);

        let sessions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1 AND expires_at > ?2",
                rusqlite::params![user.user_id, now_epoch()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let tenant: String = conn
            .query_row(
                "SELECT name FROM tenants WHERE id = ?1",
                rusqlite::params![user.tenant_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| user.tenant_id.clone());

        (mfa, sessions, tenant)
    });

    let mfa_badge = if mfa_enabled {
        r#"<span class="badge badge-ok">Enabled</span>"#
    } else {
        r#"<span class="badge badge-warn">Disabled</span>"#
    };

    let body = format!(
        r#"<h2>Account overview</h2>
<dl>
  <dt>Email</dt><dd>{email}</dd>
  <dt>Tenant</dt><dd>{tenant}</dd>
  <dt>Two-factor auth</dt><dd>{mfa_badge}</dd>
  <dt>Active sessions</dt><dd>{sessions}</dd>
</dl>
<nav>
  <ul>
    <li><a href="/account/password">Change password</a></li>
    <li><a href="/account/mfa">Two-factor authentication</a></li>
    <li><a href="/account/sessions">Active sessions</a></li>
    <li><a href="/logout">Sign out</a></li>
  </ul>
</nav>"#,
        email = esc_html(&user.email),
        tenant = esc_html(&tenant_name),
        sessions = session_count,
    );

    Html(account_page("Account", &body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/password
// ---------------------------------------------------------------------------

async fn password_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let _ = user; // session verified

    let body = format!(
        r#"<h2>Change password</h2>
<form method="post" action="/account/password">
  <input type="hidden" name="_csrf" value="{csrf}">
  <label for="current_password">Current password</label>
  <input type="password" id="current_password" name="current_password" required>
  <label for="new_password">New password</label>
  <input type="password" id="new_password" name="new_password" required minlength="8">
  <button type="submit">Change password</button>
</form>
<p><a href="/account">Back to account</a></p>"#,
        csrf = esc_html(&csrf),
    );

    Html(account_page("Change password", &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/password
// ---------------------------------------------------------------------------

async fn password_submit(headers: HeaderMap, Form(form): Form<PasswordForm>) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let current = form.current_password.as_deref().unwrap_or("");
    let new_pw = form.new_password.as_deref().unwrap_or("");
    let ip = extract_ip(&headers);

    if current.is_empty() || new_pw.is_empty() {
        return account_error("Change password", "All fields are required.");
    }
    if new_pw.len() < 8 {
        return account_error("Change password", "New password must be at least 8 characters.");
    }

    // Load current hash
    let stored_hash = with_db(|conn| {
        conn.query_row(
            "SELECT password_hash FROM users WHERE id = ?1",
            rusqlite::params![user.user_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    });

    let stored_hash = match stored_hash {
        Some(h) => h,
        None => return account_error("Change password", "User not found."),
    };

    if !verify_password(&stored_hash, current).unwrap_or(false) {
        return account_error("Change password", "Current password is incorrect.");
    }

    // Check new != old
    if verify_password(&stored_hash, new_pw).unwrap_or(false) {
        return account_error("Change password", "New password must be different from current.");
    }

    let new_hash = match hash_password(new_pw) {
        Ok(h) => h,
        Err(_) => return account_error("Change password", "Failed to hash password."),
    };

    with_db(|conn| {
        conn.execute(
            "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_hash, now_epoch(), user.user_id],
        )
        .ok();
        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.user_id),
            USER_PASSWORD_CHANGED,
            Some(&ip),
            None,
        );
    });

    let body = r#"<h2>Change password</h2>
<div class="success">Password changed successfully.</div>
<p><a href="/account">Back to account</a></p>"#;

    Html(account_page("Change password", body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/mfa
// ---------------------------------------------------------------------------

async fn mfa_status(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let mfa_enabled = with_db(|conn| {
        conn.query_row(
            "SELECT mfa_enabled FROM users WHERE id = ?1",
            rusqlite::params![user.user_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(false)
    });

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();

    let body = if mfa_enabled {
        format!(
            r#"<h2>Two-factor authentication</h2>
<p>Two-factor authentication is <strong>enabled</strong>.</p>
<form method="post" action="/account/mfa/disable">
  <input type="hidden" name="_csrf" value="{csrf}">
  <label for="password">Confirm your password to disable</label>
  <input type="password" id="password" name="password" required>
  <button type="submit" class="danger">Disable two-factor auth</button>
</form>
<p><a href="/account">Back to account</a></p>"#,
            csrf = esc_html(&csrf),
        )
    } else {
        r#"<h2>Two-factor authentication</h2>
<p>Two-factor authentication is <strong>not enabled</strong>.</p>
<p><a href="/account/mfa/setup">Set up two-factor authentication</a></p>
<p><a href="/account">Back to account</a></p>"#
            .to_string()
    };

    Html(account_page("Two-factor authentication", &body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/mfa/setup
// ---------------------------------------------------------------------------

async fn mfa_setup_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let (secret, otpauth_uri) = generate_totp_secret(&user.email, "AkurAI");

    let body = format!(
        r#"<h2>Set up two-factor authentication</h2>
<p>Scan this code with your authenticator app, or enter the key manually:</p>
<p class="mono"><code>{secret}</code></p>
<p class="small">otpauth URI: <code>{uri}</code></p>
<form method="post" action="/account/mfa/setup">
  <input type="hidden" name="_csrf" value="{csrf}">
  <input type="hidden" name="secret" value="{secret}">
  <label for="code">Enter verification code</label>
  <input type="text" id="code" name="code" inputmode="numeric" pattern="[0-9]{{6}}" autocomplete="one-time-code" required autofocus>
  <button type="submit">Verify and enable</button>
</form>
<p><a href="/account/mfa">Cancel</a></p>"#,
        csrf = esc_html(&csrf),
        secret = esc_html(&secret),
        uri = esc_html(&otpauth_uri),
    );

    Html(account_page("Set up 2FA", &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/mfa/setup
// ---------------------------------------------------------------------------

async fn mfa_setup_submit(headers: HeaderMap, Form(form): Form<MfaSetupForm>) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let code = form.code.as_deref().unwrap_or("").trim().to_string();
    let secret = form.secret.as_deref().unwrap_or("").trim().to_string();
    let ip = extract_ip(&headers);

    if code.is_empty() || secret.is_empty() {
        return account_error("Set up 2FA", "Code and secret are required.");
    }

    // Verify the code against the provided secret (±1 window configured in totp module)
    let valid = verify_totp(&secret, &code);
    if !valid {
        return account_error("Set up 2FA", "Invalid verification code. Please try again.");
    }

    // Save MFA secret and enable MFA
    let now = now_epoch();
    with_db(|conn| {
        conn.execute(
            "UPDATE users SET mfa_secret = ?1, mfa_enabled = 1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![secret, now, user.user_id],
        )
        .ok();

        // Delete any existing backup codes
        conn.execute(
            "DELETE FROM backup_codes WHERE user_id = ?1",
            rusqlite::params![user.user_id],
        )
        .ok();

        // Generate 10 backup codes
        let codes = generate_backup_codes(10);
        for raw_code in &codes {
            let id = generate_secure_token(16);
            let code_hash = sha256(raw_code);
            conn.execute(
                "INSERT INTO backup_codes (id, user_id, code_hash, used) VALUES (?1, ?2, ?3, 0)",
                rusqlite::params![id, user.user_id, code_hash],
            )
            .ok();
        }

        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.user_id),
            USER_MFA_ENABLED,
            Some(&ip),
            None,
        );

        // Display backup codes once
        let codes_html: String = codes
            .iter()
            .map(|c| format!("<li><code>{}</code></li>", esc_html(c)))
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            r#"<h2>Two-factor authentication enabled</h2>
<div class="success">Two-factor authentication has been enabled.</div>
<h3>Backup codes</h3>
<p><strong>Save these codes somewhere safe.</strong> Each can be used once if you lose your authenticator.</p>
<ul class="backup-codes">
{codes_html}
</ul>
<p><a href="/account">Back to account</a></p>"#,
        );

        Html(account_page("2FA enabled", &body)).into_response()
    })
}

// ---------------------------------------------------------------------------
// POST /account/mfa/disable
// ---------------------------------------------------------------------------

async fn mfa_disable(headers: HeaderMap, Form(form): Form<MfaDisableForm>) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let password = form.password.as_deref().unwrap_or("");
    let ip = extract_ip(&headers);

    if password.is_empty() {
        return account_error("Two-factor authentication", "Password is required.");
    }

    // Verify current password
    let stored_hash = with_db(|conn| {
        conn.query_row(
            "SELECT password_hash FROM users WHERE id = ?1",
            rusqlite::params![user.user_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    });

    let stored_hash = match stored_hash {
        Some(h) => h,
        None => return account_error("Two-factor authentication", "User not found."),
    };

    if !verify_password(&stored_hash, password).unwrap_or(false) {
        return account_error("Two-factor authentication", "Password is incorrect.");
    }

    with_db(|conn| {
        conn.execute(
            "UPDATE users SET mfa_secret = NULL, mfa_enabled = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now_epoch(), user.user_id],
        )
        .ok();

        conn.execute(
            "DELETE FROM backup_codes WHERE user_id = ?1",
            rusqlite::params![user.user_id],
        )
        .ok();

        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.user_id),
            USER_MFA_DISABLED,
            Some(&ip),
            None,
        );
    });

    let body = r#"<h2>Two-factor authentication</h2>
<div class="success">Two-factor authentication has been disabled.</div>
<p><a href="/account">Back to account</a></p>"#;

    Html(account_page("Two-factor authentication", body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/sessions
// ---------------------------------------------------------------------------

async fn sessions_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let now = now_epoch();

    let sessions = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, ip, user_agent, created_at, expires_at
                 FROM sessions
                 WHERE user_id = ?1 AND expires_at > ?2
                 ORDER BY created_at DESC",
            )
            .unwrap();
        let rows = stmt
            .query_map(rusqlite::params![user.user_id, now], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    ip: row.get(1)?,
                    user_agent: row.get(2)?,
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        rows
    });

    let mut rows_html = String::new();
    for s in &sessions {
        let is_current = s.id == user.session_id;
        let id_display = if s.id.len() > 12 {
            format!("{}...", &s.id[..12])
        } else {
            s.id.clone()
        };
        let current_label = if is_current { " (current)" } else { "" };
        let revoke_btn = if is_current {
            String::new()
        } else {
            format!(
                r#"<form method="post" action="/account/sessions/{id}/revoke" style="display:inline">
  <input type="hidden" name="_csrf" value="{csrf}">
  <button type="submit" class="small danger">Revoke</button>
</form>"#,
                id = esc_html(&s.id),
                csrf = esc_html(&csrf),
            )
        };

        rows_html.push_str(&format!(
            r#"<tr{class}>
  <td><code>{id_display}</code>{current_label}</td>
  <td>{ip}</td>
  <td>{ua}</td>
  <td>{created}</td>
  <td>{expires}</td>
  <td>{revoke_btn}</td>
</tr>"#,
            class = if is_current {
                r#" class="current""#
            } else {
                ""
            },
            id_display = esc_html(&id_display),
            ip = esc_html(&s.ip),
            ua = esc_html(&truncate_ua(&s.user_agent)),
            created = format_epoch(s.created_at),
            expires = format_epoch(s.expires_at),
        ));
    }

    let body = format!(
        r#"<h2>Active sessions</h2>
<table>
  <thead>
    <tr><th>ID</th><th>IP</th><th>User agent</th><th>Created</th><th>Expires</th><th></th></tr>
  </thead>
  <tbody>
    {rows_html}
  </tbody>
</table>
<form method="post" action="/account/sessions/revoke-all">
  <input type="hidden" name="_csrf" value="{csrf}">
  <button type="submit" class="danger">Revoke all other sessions</button>
</form>
<p><a href="/account">Back to account</a></p>"#,
        csrf = esc_html(&csrf),
    );

    Html(account_page("Active sessions", &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/sessions/:id/revoke
// ---------------------------------------------------------------------------

async fn session_revoke(
    headers: HeaderMap,
    Path(target_id): Path<String>,
) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let ip = extract_ip(&headers);

    // Don't allow revoking current session via this endpoint
    if target_id == user.session_id {
        return redirect("/account/sessions");
    }

    with_db(|conn| {
        conn.execute(
            "DELETE FROM sessions WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![target_id, user.user_id],
        )
        .ok();

        let meta = serde_json::json!({"revoked_session": target_id});
        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.user_id),
            SESSION_REVOKED,
            Some(&ip),
            Some(&meta),
        );
    });

    redirect("/account/sessions")
}

// ---------------------------------------------------------------------------
// POST /account/sessions/revoke-all
// ---------------------------------------------------------------------------

async fn sessions_revoke_all(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let ip = extract_ip(&headers);

    with_db(|conn| {
        conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND id != ?2",
            rusqlite::params![user.user_id, user.session_id],
        )
        .ok();

        let meta = serde_json::json!({"action": "revoke_all_other"});
        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.user_id),
            SESSION_REVOKED,
            Some(&ip),
            Some(&meta),
        );
    });

    redirect("/account/sessions")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct SessionRow {
    id: String,
    ip: String,
    user_agent: String,
    created_at: i64,
    expires_at: i64,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

fn get_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    get_cookie(headers, "_csrf")
}

fn extract_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default()
}

fn redirect(path: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, path.to_string())]).into_response()
}

fn account_error(title: &str, message: &str) -> Response {
    let body = format!(
        r#"<h2>{title}</h2>
<div class="error">{message}</div>
<p><a href="/account">Back to account</a></p>"#,
        title = esc_html(title),
        message = esc_html(message),
    );
    Html(account_page(title, &body)).into_response()
}

fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn truncate_ua(ua: &str) -> String {
    if ua.len() > 80 {
        format!("{}...", &ua[..80])
    } else {
        ua.to_string()
    }
}

fn format_epoch(ts: i64) -> String {
    // Simple UTC timestamp display
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}
