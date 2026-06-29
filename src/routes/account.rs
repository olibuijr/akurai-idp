use axum::{
    Form, Router,
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::db::with_db;
use crate::lib::audit::{
    SESSION_REVOKED, USER_MFA_DISABLED, USER_MFA_ENABLED, USER_PASSWORD_CHANGED, log_audit_event,
};
use crate::lib::crypto::{generate_secure_token, sha256};
use crate::lib::html::{Locale, account_page, esc_html, t};
use crate::lib::password::{hash_password, verify_password};
use crate::lib::totp::{generate_backup_codes, generate_totp_secret, verify_totp};

pub fn router() -> Router {
    Router::new()
        .route("/account", get(account_overview))
        .route(
            "/account/password",
            get(password_page).post(password_submit),
        )
        .route("/account/mfa", get(mfa_status))
        .route(
            "/account/mfa/setup",
            get(mfa_setup_page).post(mfa_setup_submit),
        )
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
    let locale = locale_from_headers(&headers);

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

    let lbl_enabled = t(locale, "Virkt", "Enabled");
    let lbl_disabled = t(locale, "Óvirkt", "Disabled");
    let mfa_badge = if mfa_enabled {
        format!(r#"<span class="badge badge-ok">{lbl_enabled}</span>"#)
    } else {
        format!(r#"<span class="badge badge-warn">{lbl_disabled}</span>"#)
    };

    let agent_url = &config::get().agent_public_url;
    let body = format!(
        r#"<h2>{h_overview}</h2>
<dl>
  <dt>{lbl_email}</dt><dd>{email}</dd>
  <dt>{lbl_tenant}</dt><dd>{tenant}</dd>
  <dt>{lbl_mfa}</dt><dd>{mfa_badge}</dd>
  <dt>{lbl_sessions}</dt><dd>{sessions}</dd>
</dl>
<nav>
  <ul>
    <li><a href="/account/password">{lbl_change_pw}</a></li>
    <li><a href="/account/mfa">{lbl_2fa}</a></li>
    <li><a href="/account/sessions">{lbl_active_sessions}</a></li>
    <li><a href="{agent_url}">{lbl_agent}</a></li>
    <li><a href="/logout">{lbl_signout}</a></li>
  </ul>
</nav>"#,
        h_overview = t(locale, "Yfirlit reiknings", "Account overview"),
        lbl_email = t(locale, "Netfang", "Email"),
        lbl_tenant = t(locale, "Leigjandi", "Tenant"),
        lbl_mfa = t(locale, "Tvíþátta auðkenning", "Two-factor auth"),
        lbl_sessions = t(locale, "Virkar lotur", "Active sessions"),
        lbl_change_pw = t(locale, "Breyta lykilorði", "Change password"),
        lbl_2fa = t(locale, "Tvíþátta auðkenning", "Two-factor authentication"),
        lbl_active_sessions = t(locale, "Virkar lotur", "Active sessions"),
        lbl_agent = t(locale, "Stjórnborð fulltrúa", "Agent console"),
        lbl_signout = t(locale, "Skrá út", "Sign out"),
        email = esc_html(&user.email),
        tenant = esc_html(&tenant_name),
        sessions = session_count,
        mfa_badge = mfa_badge,
        agent_url = esc_html(agent_url),
    );

    Html(account_page(
        locale,
        t(locale, "Reikningur", "Account"),
        &body,
    ))
    .into_response()
}

// ---------------------------------------------------------------------------
// GET /account/password
// ---------------------------------------------------------------------------

async fn password_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale = locale_from_headers(&headers);

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let _ = user; // session verified

    let page_title = t(locale, "Breyta lykilorði", "Change password");
    let body = format!(
        r#"<h2>{page_title}</h2>
<form method="post" action="/account/password">
  <input type="hidden" name="_csrf" value="{csrf}">
  <label for="current_password">{lbl_current}</label>
  <input type="password" id="current_password" name="current_password" required>
  <label for="new_password">{lbl_new}</label>
  <input type="password" id="new_password" name="new_password" required minlength="8">
  <button type="submit">{lbl_submit}</button>
</form>
<p><a href="/account">{lbl_back}</a></p>"#,
        page_title = page_title,
        csrf = esc_html(&csrf),
        lbl_current = t(locale, "Núverandi lykilorð", "Current password"),
        lbl_new = t(locale, "Nýtt lykilorð", "New password"),
        lbl_submit = t(locale, "Breyta lykilorði", "Change password"),
        lbl_back = t(locale, "Aftur í reikning", "Back to account"),
    );

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/password
// ---------------------------------------------------------------------------

async fn password_submit(headers: HeaderMap, Form(form): Form<PasswordForm>) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale = locale_from_headers(&headers);

    let current = form.current_password.as_deref().unwrap_or("");
    let new_pw = form.new_password.as_deref().unwrap_or("");
    let ip = extract_ip(&headers);

    if current.is_empty() || new_pw.is_empty() {
        return account_error(locale, "Change password", "All fields are required.");
    }
    if new_pw.len() < 8 {
        return account_error(
            locale,
            "Change password",
            "New password must be at least 8 characters.",
        );
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
        None => return account_error(locale, "Change password", "User not found."),
    };

    if !verify_password(&stored_hash, current).unwrap_or(false) {
        return account_error(locale, "Change password", "Current password is incorrect.");
    }

    // Check new != old
    if verify_password(&stored_hash, new_pw).unwrap_or(false) {
        return account_error(
            locale,
            "Change password",
            "New password must be different from current.",
        );
    }

    let new_hash = match hash_password(new_pw) {
        Ok(h) => h,
        Err(_) => return account_error(locale, "Change password", "Failed to hash password."),
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

    let page_title = t(locale, "Breyta lykilorði", "Change password");
    let body = format!(
        r#"<h2>{page_title}</h2>
<div class="success">{lbl_success}</div>
<p><a href="/account">{lbl_back}</a></p>"#,
        page_title = page_title,
        lbl_success = t(
            locale,
            "Lykilorði hefur verið breytt.",
            "Password changed successfully."
        ),
        lbl_back = t(locale, "Aftur í reikning", "Back to account"),
    );

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/mfa
// ---------------------------------------------------------------------------

async fn mfa_status(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale = locale_from_headers(&headers);

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
    let page_title = t(locale, "Tvíþátta auðkenning", "Two-factor authentication");

    let body = if mfa_enabled {
        format!(
            r#"<h2>{page_title}</h2>
<p>{lbl_is_enabled}</p>
<form method="post" action="/account/mfa/disable">
  <input type="hidden" name="_csrf" value="{csrf}">
  <label for="password">{lbl_confirm_pw}</label>
  <input type="password" id="password" name="password" required>
  <button type="submit" class="danger">{lbl_disable}</button>
</form>
<p><a href="/account">{lbl_back}</a></p>"#,
            page_title = page_title,
            csrf = esc_html(&csrf),
            lbl_is_enabled = t(
                locale,
                "Tvíþátta auðkenning er <strong>virk</strong>.",
                "Two-factor authentication is <strong>enabled</strong>."
            ),
            lbl_confirm_pw = t(
                locale,
                "Staðfestu lykilorðið þitt til að afvirkja",
                "Confirm your password to disable"
            ),
            lbl_disable = t(
                locale,
                "Afvirkja tvíþátta auðkenningu",
                "Disable two-factor auth"
            ),
            lbl_back = t(locale, "Aftur í reikning", "Back to account"),
        )
    } else {
        format!(
            r#"<h2>{page_title}</h2>
<p>{lbl_not_enabled}</p>
<p><a href="/account/mfa/setup">{lbl_setup}</a></p>
<p><a href="/account">{lbl_back}</a></p>"#,
            page_title = page_title,
            lbl_not_enabled = t(
                locale,
                "Tvíþátta auðkenning er <strong>ekki virk</strong>.",
                "Two-factor authentication is <strong>not enabled</strong>."
            ),
            lbl_setup = t(
                locale,
                "Setja upp tvíþátta auðkenningu",
                "Set up two-factor authentication"
            ),
            lbl_back = t(locale, "Aftur í reikning", "Back to account"),
        )
    };

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/mfa/setup
// ---------------------------------------------------------------------------

async fn mfa_setup_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale =
        Locale::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));

    let csrf = get_csrf_cookie(&headers).unwrap_or_default();
    let (secret, otpauth_uri) = generate_totp_secret(&user.email, "AkurAI");

    let page_title = t(locale, "Setja upp 2FA", "Set up 2FA");
    let body = format!(
        r#"<h2>{h_setup}</h2>
<p>{lbl_scan}</p>
<p class="mono"><code>{secret}</code></p>
<p class="small">otpauth URI: <code>{uri}</code></p>
<form method="post" action="/account/mfa/setup">
  <input type="hidden" name="_csrf" value="{csrf}">
  <input type="hidden" name="secret" value="{secret}">
  <label for="code">{lbl_code}</label>
  <input type="text" id="code" name="code" inputmode="numeric" pattern="[0-9]{{6}}" autocomplete="one-time-code" required autofocus>
  <button type="submit">{lbl_verify}</button>
</form>
<p><a href="/account/mfa">{lbl_cancel}</a></p>"#,
        h_setup = t(
            locale,
            "Setja upp tvíþátta auðkenningu",
            "Set up two-factor authentication"
        ),
        lbl_scan = t(
            locale,
            "Skannaðu þennan kóða með auðkenningarforritinu þínu, eða sláðu inn lykilinn handvirkt:",
            "Scan this code with your authenticator app, or enter the key manually:"
        ),
        lbl_code = t(
            locale,
            "Sláðu inn staðfestingarkóða",
            "Enter verification code"
        ),
        lbl_verify = t(locale, "Staðfesta og virkja", "Verify and enable"),
        lbl_cancel = t(locale, "Hætta við", "Cancel"),
        csrf = esc_html(&csrf),
        secret = esc_html(&secret),
        uri = esc_html(&otpauth_uri),
    );

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/mfa/setup
// ---------------------------------------------------------------------------

async fn mfa_setup_submit(headers: HeaderMap, Form(form): Form<MfaSetupForm>) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale = locale_from_headers(&headers);

    let code = form.code.as_deref().unwrap_or("").trim().to_string();
    let secret = form.secret.as_deref().unwrap_or("").trim().to_string();
    let ip = extract_ip(&headers);

    if code.is_empty() || secret.is_empty() {
        return account_error(locale, "Set up 2FA", "Code and secret are required.");
    }

    // Verify the code against the provided secret (±1 window configured in totp module)
    let valid = verify_totp(&secret, &code);
    if !valid {
        return account_error(
            locale,
            "Set up 2FA",
            "Invalid verification code. Please try again.",
        );
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

        let page_title = t(locale, "2FA virkt", "2FA enabled");
        let body = format!(
            r#"<h2>{h_enabled}</h2>
<div class="success">{lbl_success}</div>
<h3>{h_backup}</h3>
<p><strong>{lbl_save}</strong> {lbl_each}</p>
<ul class="backup-codes">
{codes_html}
</ul>
<p><a href="/account">{lbl_back}</a></p>"#,
            h_enabled = t(
                locale,
                "Tvíþátta auðkenning virk",
                "Two-factor authentication enabled"
            ),
            lbl_success = t(
                locale,
                "Tvíþátta auðkenning hefur verið virkjuð.",
                "Two-factor authentication has been enabled."
            ),
            h_backup = t(locale, "Varakóðar", "Backup codes"),
            lbl_save = t(
                locale,
                "Vistaðu þessa kóða á öruggum stað.",
                "Save these codes somewhere safe."
            ),
            lbl_each = t(
                locale,
                "Hægt er að nota hvern eitt sinni ef þú missir auðkenningartækið.",
                "Each can be used once if you lose your authenticator."
            ),
            lbl_back = t(locale, "Aftur í reikning", "Back to account"),
            codes_html = codes_html,
        );

        Html(account_page(locale, page_title, &body)).into_response()
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
    let locale = locale_from_headers(&headers);

    let password = form.password.as_deref().unwrap_or("");
    let ip = extract_ip(&headers);

    if password.is_empty() {
        return account_error(locale, "Two-factor authentication", "Password is required.");
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
        None => return account_error(locale, "Two-factor authentication", "User not found."),
    };

    if !verify_password(&stored_hash, password).unwrap_or(false) {
        return account_error(
            locale,
            "Two-factor authentication",
            "Password is incorrect.",
        );
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

    let page_title = t(locale, "Tvíþátta auðkenning", "Two-factor authentication");
    let body = format!(
        r#"<h2>{page_title}</h2>
<div class="success">{lbl_disabled}</div>
<p><a href="/account">{lbl_back}</a></p>"#,
        page_title = page_title,
        lbl_disabled = t(
            locale,
            "Tvíþátta auðkenning hefur verið afvirkjuð.",
            "Two-factor authentication has been disabled."
        ),
        lbl_back = t(locale, "Aftur í reikning", "Back to account"),
    );

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// GET /account/sessions
// ---------------------------------------------------------------------------

async fn sessions_page(headers: HeaderMap) -> Response {
    let user = match require_session(&headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let locale = locale_from_headers(&headers);

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

        stmt.query_map(rusqlite::params![user.user_id, now], |row| {
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
        .collect::<Vec<_>>()
    });

    let lbl_current = t(locale, " (núverandi)", " (current)");
    let lbl_revoke = t(locale, "Afturkalla", "Revoke");

    let mut rows_html = String::new();
    for s in &sessions {
        let is_current = s.id == user.session_id;
        let id_display = if s.id.len() > 12 {
            format!("{}...", &s.id[..12])
        } else {
            s.id.clone()
        };
        let current_label = if is_current { lbl_current } else { "" };
        let revoke_btn = if is_current {
            String::new()
        } else {
            format!(
                r#"<form method="post" action="/account/sessions/{id}/revoke" style="display:inline">
  <input type="hidden" name="_csrf" value="{csrf}">
  <button type="submit" class="small danger">{lbl}</button>
</form>"#,
                id = esc_html(&s.id),
                csrf = esc_html(&csrf),
                lbl = lbl_revoke,
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
            current_label = current_label,
            ip = esc_html(&s.ip),
            ua = esc_html(&truncate_ua(&s.user_agent)),
            created = format_epoch(s.created_at),
            expires = format_epoch(s.expires_at),
            revoke_btn = revoke_btn,
        ));
    }

    let page_title = t(locale, "Virkar lotur", "Active sessions");
    let body = format!(
        r#"<h2>{page_title}</h2>
<table>
  <thead>
    <tr><th>{th_id}</th><th>{th_ip}</th><th>{th_ua}</th><th>{th_created}</th><th>{th_expires}</th><th></th></tr>
  </thead>
  <tbody>
    {rows_html}
  </tbody>
</table>
<form method="post" action="/account/sessions/revoke-all">
  <input type="hidden" name="_csrf" value="{csrf}">
  <button type="submit" class="danger">{lbl_revoke_all}</button>
</form>
<p><a href="/account">{lbl_back}</a></p>"#,
        page_title = page_title,
        th_id = t(locale, "Auðkenni", "ID"),
        th_ip = t(locale, "IP-tala", "IP"),
        th_ua = t(locale, "Notendaforrit", "User agent"),
        th_created = t(locale, "Stofnað", "Created"),
        th_expires = t(locale, "Rennur út", "Expires"),
        lbl_revoke_all = t(
            locale,
            "Afturkalla allar aðrar lotur",
            "Revoke all other sessions"
        ),
        lbl_back = t(locale, "Aftur í reikning", "Back to account"),
        csrf = esc_html(&csrf),
        rows_html = rows_html,
    );

    Html(account_page(locale, page_title, &body)).into_response()
}

// ---------------------------------------------------------------------------
// POST /account/sessions/:id/revoke
// ---------------------------------------------------------------------------

async fn session_revoke(headers: HeaderMap, Path(target_id): Path<String>) -> Response {
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
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, path.to_string())],
    )
        .into_response()
}

fn locale_from_headers(headers: &HeaderMap) -> Locale {
    Locale::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))
}

fn account_error(locale: Locale, title: &str, message: &str) -> Response {
    let lbl_back = t(locale, "Aftur í reikning", "Back to account");
    let body = format!(
        r#"<h2>{title}</h2>
<div class="error">{message}</div>
<p><a href="/account">{lbl_back}</a></p>"#,
        title = esc_html(title),
        message = esc_html(message),
        lbl_back = lbl_back,
    );
    Html(account_page(locale, title, &body)).into_response()
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
