use axum::{
    Form, Router,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::db::with_db;
use crate::lib::audit::{
    SESSION_CREATED, USER_LOCKED, USER_LOGIN, USER_LOGIN_FAILED, log_audit_event,
};
use crate::lib::crypto::{generate_secure_token, sha256};
use crate::lib::html::{Locale, auth_page, esc_html, t};
use crate::lib::password::verify_password;
use crate::lib::totp::verify_totp;

pub fn router() -> Router {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/mfa", get(mfa_page).post(mfa_submit))
        .route("/logout", get(logout).post(logout))
}

// ---------------------------------------------------------------------------
// Query / form structs
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct LoginQuery {
    error: Option<String>,
    return_to: Option<String>,
}

#[derive(Deserialize)]
struct LoginForm {
    email: Option<String>,
    password: Option<String>,
    return_to: Option<String>,
    _csrf: Option<String>,
}

#[derive(Deserialize)]
struct MfaForm {
    code: Option<String>,
    _csrf: Option<String>,
}

// ---------------------------------------------------------------------------
// GET /login
// ---------------------------------------------------------------------------

async fn login_page(Query(q): Query<LoginQuery>, headers: HeaderMap) -> impl IntoResponse {
    let locale =
        Locale::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let return_to = q.return_to.as_deref().unwrap_or("");
    let error_html = match &q.error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, esc_html(msg)),
        None => String::new(),
    };

    let lbl_email = t(locale, "Netfang", "Email");
    let lbl_password = t(locale, "Lykilorð", "Password");
    let lbl_submit = t(locale, "Skrá inn", "Sign in");
    let page_title = t(locale, "Skrá inn", "Sign in");

    let body = format!(
        r#"{error_html}
<form method="post" action="/login">
  <input type="hidden" name="_csrf" value="{csrf}">
  <input type="hidden" name="return_to" value="{return_to}">
  <label for="email">{lbl_email}</label>
  <input type="email" id="email" name="email" required autofocus>
  <label for="password">{lbl_password}</label>
  <input type="password" id="password" name="password" required>
  <button type="submit">{lbl_submit}</button>
</form>"#,
        csrf = esc_html(&csrf),
        return_to = esc_html(return_to),
        lbl_email = lbl_email,
        lbl_password = lbl_password,
        lbl_submit = lbl_submit,
    );

    Html(auth_page(locale, page_title, &body))
}

// ---------------------------------------------------------------------------
// POST /login
// ---------------------------------------------------------------------------

async fn login_submit(headers: HeaderMap, Form(form): Form<LoginForm>) -> Response {
    let email = form.email.as_deref().unwrap_or("").trim().to_lowercase();
    let password = form.password.as_deref().unwrap_or("");
    let return_to = safe_return_to(form.return_to.as_deref());
    let ip = extract_ip(&headers);

    if email.is_empty() || password.is_empty() {
        return login_redirect("Email and password are required", &return_to);
    }

    // Global lookup across tenants (email not globally unique, but we pick first match)
    let user = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, password_hash, mfa_enabled,
                        locked_until, failed_attempts
                 FROM users WHERE LOWER(email) = ?1 LIMIT 1",
            )
            .ok()?;
        stmt.query_row(rusqlite::params![email], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                password_hash: row.get(2)?,
                mfa_enabled: row.get::<_, i64>(3)? != 0,
                locked_until: row.get(4)?,
                failed_attempts: row.get::<_, i64>(5)?,
            })
        })
        .ok()
    });

    let user = match user {
        Some(u) => u,
        None => {
            // Don't reveal whether the user exists
            return login_redirect("Invalid email or password", &return_to);
        }
    };

    let now = now_epoch();

    // Check lockout
    if let Some(locked_until) = user.locked_until {
        if locked_until > now {
            return login_redirect("Account temporarily locked. Try again later.", &return_to);
        }
    }

    // Verify password
    let pw_ok = verify_password(&user.password_hash, password).unwrap_or(false);
    if !pw_ok {
        let new_attempts = user.failed_attempts + 1;
        with_db(|conn| {
            if new_attempts >= 5 {
                let lock_until = now + 15 * 60; // 15 minutes
                conn.execute(
                    "UPDATE users SET failed_attempts = ?1, locked_until = ?2, updated_at = ?3 WHERE id = ?4",
                    rusqlite::params![new_attempts, lock_until, now, user.id],
                )
                .ok();
                log_audit_event(
                    conn,
                    Some(&user.tenant_id),
                    Some(&user.id),
                    USER_LOCKED,
                    Some(&ip),
                    None,
                );
            } else {
                conn.execute(
                    "UPDATE users SET failed_attempts = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![new_attempts, now, user.id],
                )
                .ok();
            }
            log_audit_event(
                conn,
                Some(&user.tenant_id),
                Some(&user.id),
                USER_LOGIN_FAILED,
                Some(&ip),
                None,
            );
        });
        return login_redirect("Invalid email or password", &return_to);
    }

    // Password correct — reset failed attempts
    with_db(|conn| {
        conn.execute(
            "UPDATE users SET failed_attempts = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, user.id],
        )
        .ok();
    });

    // MFA check
    if user.mfa_enabled {
        let token = generate_secure_token(32);
        let cookie_val = format!("{}:{}", token, user.id);
        let cookie = format!(
            "idp_mfa_pending={cookie_val}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=300"
        );
        let redirect_url = format!("/mfa?return_to={}", urlencoded(&return_to));
        return (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, redirect_url),
                (header::SET_COOKIE, cookie),
            ],
        )
            .into_response();
    }

    // No MFA — create session directly
    create_session_response(
        &user.id,
        &user.tenant_id,
        &ip,
        &extract_ua(&headers),
        &return_to,
    )
}

// ---------------------------------------------------------------------------
// GET /mfa
// ---------------------------------------------------------------------------

async fn mfa_page(Query(q): Query<LoginQuery>, headers: HeaderMap) -> impl IntoResponse {
    let locale =
        Locale::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let error_html = match &q.error {
        Some(msg) => format!(r#"<div class="error">{}</div>"#, esc_html(msg)),
        None => String::new(),
    };

    let lbl_code = t(locale, "Auðkenniskóði", "Authentication code");
    let lbl_hint = t(
        locale,
        "Sláðu inn 6 stafa auðkenniskóðann þinn eða varakóða.",
        "Enter your 6-digit authenticator code or a backup code.",
    );
    let lbl_submit = t(locale, "Staðfesta", "Verify");
    let page_title = t(locale, "Tvíþátta auðkenning", "Two-factor authentication");

    let body = format!(
        r#"{error_html}
<form method="post" action="/mfa">
  <input type="hidden" name="_csrf" value="{csrf}">
  <label for="code">{lbl_code}</label>
  <input type="text" id="code" name="code" inputmode="numeric" pattern="[0-9]*" autocomplete="one-time-code" required autofocus>
  <p class="hint">{lbl_hint}</p>
  <button type="submit">{lbl_submit}</button>
</form>"#,
        csrf = esc_html(&csrf),
        lbl_code = lbl_code,
        lbl_hint = lbl_hint,
        lbl_submit = lbl_submit,
    );

    Html(auth_page(locale, page_title, &body))
}

// ---------------------------------------------------------------------------
// POST /mfa
// ---------------------------------------------------------------------------

async fn mfa_submit(headers: HeaderMap, Form(form): Form<MfaForm>) -> Response {
    let code = form.code.as_deref().unwrap_or("").trim().to_string();
    let ip = extract_ip(&headers);

    // Read MFA pending cookie
    let pending = match get_cookie(&headers, "idp_mfa_pending") {
        Some(v) => v,
        None => return login_redirect("MFA session expired. Please log in again.", ""),
    };

    // Format: {token}:{user_id}
    let user_id = match pending.split_once(':').map(|(_, uid)| uid.to_string()) {
        Some(uid) if !uid.is_empty() => uid,
        _ => return login_redirect("Invalid MFA session.", ""),
    };

    // Load user
    let user = with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, tenant_id, mfa_secret FROM users WHERE id = ?1")
            .ok()?;
        stmt.query_row(rusqlite::params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .ok()
    });

    let (uid, tenant_id, mfa_secret) = match user {
        Some(u) => u,
        None => return login_redirect("Invalid MFA session.", ""),
    };

    let secret = match mfa_secret {
        Some(s) if !s.is_empty() => s,
        _ => return login_redirect("MFA not configured.", ""),
    };

    // Try TOTP first (±1 window configured in totp module)
    let totp_ok = verify_totp(&secret, &code);

    if !totp_ok {
        // Try backup codes
        let code_hash = sha256(&code);
        let backup_ok = with_db(|conn| {
            let changed = conn
                .execute(
                    "UPDATE backup_codes SET used = 1 WHERE user_id = ?1 AND code_hash = ?2 AND used = 0",
                    rusqlite::params![uid, code_hash],
                )
                .unwrap_or(0);
            changed > 0
        });

        if !backup_ok {
            // Clear the pending cookie on repeated failure? No — let them retry within the 5 min window.
            let redirect = format!(
                "/mfa?error={}",
                urlencoded("Invalid code. Please try again.")
            );
            return (StatusCode::SEE_OTHER, [(header::LOCATION, redirect)]).into_response();
        }
    }

    // MFA passed — clear pending cookie, create session
    let clear_pending =
        "idp_mfa_pending=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0".to_string();
    let return_to = safe_return_to(None);
    let mut resp =
        create_session_response(&uid, &tenant_id, &ip, &extract_ua(&headers), &return_to);
    resp.headers_mut()
        .append(header::SET_COOKIE, clear_pending.parse().unwrap());
    resp
}

// ---------------------------------------------------------------------------
// GET|POST /logout
// ---------------------------------------------------------------------------

async fn logout(headers: HeaderMap) -> Response {
    let ip = extract_ip(&headers);

    if let Some(session_id) = get_cookie(&headers, "idp_session") {
        with_db(|conn| {
            // Get user info for audit before deleting
            let user_info: Option<(String, String)> = conn
                .prepare("SELECT user_id, (SELECT tenant_id FROM users WHERE id = sessions.user_id) FROM sessions WHERE id = ?1")
                .ok()
                .and_then(|mut stmt| {
                    stmt.query_row(rusqlite::params![session_id], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .ok()
                });

            conn.execute(
                "DELETE FROM sessions WHERE id = ?1",
                rusqlite::params![session_id],
            )
            .ok();

            if let Some((user_id, tenant_id)) = user_info {
                log_audit_event(
                    conn,
                    Some(&tenant_id),
                    Some(&user_id),
                    crate::lib::audit::SESSION_REVOKED,
                    Some(&ip),
                    None,
                );
            }
        });
    }

    let clear = "idp_session=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0";
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/login".to_string()),
            (header::SET_COOKIE, clear.to_string()),
        ],
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct UserRow {
    id: String,
    tenant_id: String,
    password_hash: String,
    mfa_enabled: bool,
    locked_until: Option<i64>,
    failed_attempts: i64,
}

fn create_session_response(
    user_id: &str,
    tenant_id: &str,
    ip: &str,
    user_agent: &str,
    return_to: &str,
) -> Response {
    let session_id = generate_secure_token(32);
    let now = now_epoch();
    let expires = now + 24 * 3600; // 24 hours

    with_db(|conn| {
        conn.execute(
            "INSERT INTO sessions (id, user_id, ip, user_agent, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![session_id, user_id, ip, user_agent, now, expires],
        )
        .ok();

        log_audit_event(
            conn,
            Some(tenant_id),
            Some(user_id),
            SESSION_CREATED,
            Some(ip),
            None,
        );
        log_audit_event(
            conn,
            Some(tenant_id),
            Some(user_id),
            USER_LOGIN,
            Some(ip),
            None,
        );
    });

    let cookie =
        format!("idp_session={session_id}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=86400");
    let dest = if return_to.is_empty() { "/" } else { return_to };
    let escaped_dest = esc_html(dest);
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="refresh" content="0; url={escaped_dest}">
  <title>Signing in - AkurAI ID</title>
</head>
<body>
  <p>Signing in... <a href="{escaped_dest}">Continue</a></p>
</body>
</html>"#
    );

    (
        StatusCode::OK,
        [
            (header::SET_COOKIE, cookie),
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
        ],
        html,
    )
        .into_response()
}

/// Validate return_to: must start with "/" and not "//", or match base_url origin.
fn safe_return_to(input: Option<&str>) -> String {
    let val = input.unwrap_or("/").trim();
    if val.is_empty() {
        return "/".to_string();
    }

    // Relative path: must start with "/" but not "//" (protocol-relative)
    if val.starts_with('/') && !val.starts_with("//") {
        return val.to_string();
    }

    // Absolute URL: must match our base_url origin
    let base = &config::get().base_url;
    if let Some(origin) = extract_origin(base) {
        if val.starts_with(&origin) {
            return val.to_string();
        }
    }

    "/".to_string()
}

fn extract_origin(url: &str) -> Option<String> {
    // Extract scheme + host from a URL like "https://auth.example.com/path"
    let after_scheme = url
        .strip_prefix("https://")
        .map(|rest| ("https", rest))
        .or_else(|| url.strip_prefix("http://").map(|rest| ("http", rest)))?;
    let (scheme, rest) = after_scheme;
    let host = rest.split('/').next()?;
    Some(format!("{scheme}://{host}"))
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
    // Check X-Forwarded-For, X-Real-IP, then fall back to empty
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

fn extract_ua(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn login_redirect(error: &str, return_to: &str) -> Response {
    let mut url = format!("/login?error={}", urlencoded(error));
    if !return_to.is_empty() && return_to != "/" {
        url.push_str(&format!("&return_to={}", urlencoded(return_to)));
    }
    (StatusCode::SEE_OTHER, [(header::LOCATION, url)]).into_response()
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
