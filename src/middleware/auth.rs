use axum::{
    body::Body,
    http::{Request, Response, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect},
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::db::with_db;
use crate::lib::crypto::constant_time_equal;
use crate::lib::jwt::verify_token;

/// User info attached to request extensions after auth.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub tenant_id: String,
    pub email: String,
    pub email_verified: bool,
}

/// Session info attached to request extensions after session_auth.
#[derive(Clone, Debug)]
pub struct AuthSession {
    pub session_id: String,
    pub user_id: String,
}

// ---------------------------------------------------------------------------
// 1. Session auth — cookie-based, redirects to /login on failure
// ---------------------------------------------------------------------------

pub async fn session_auth(request: Request<Body>, next: Next) -> Response<Body> {
    let session_id = match get_cookie(&request, "idp_session") {
        Some(s) => s,
        None => return Redirect::to("/login").into_response(),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let result: Option<(AuthSession, AuthUser)> = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.user_id, u.tenant_id, u.email, u.email_verified
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.id = ?1 AND s.expires_at > ?2",
            )
            .ok()?;
        stmt.query_row(rusqlite::params![session_id, now], |row| {
            let session = AuthSession {
                session_id: row.get(0)?,
                user_id: row.get(1)?,
            };
            let user = AuthUser {
                id: row.get::<_, String>(1)?,
                tenant_id: row.get(2)?,
                email: row.get(3)?,
                email_verified: row.get::<_, i64>(4)? != 0,
            };
            Ok((session, user))
        })
        .ok()
    });

    match result {
        Some((session, user)) => {
            let mut request = request;
            request.extensions_mut().insert(session);
            request.extensions_mut().insert(user);
            next.run(request).await
        }
        None => Redirect::to("/login").into_response(),
    }
}

// ---------------------------------------------------------------------------
// 1b. Optional session — populates extensions if valid, always continues
// ---------------------------------------------------------------------------

pub async fn session_optional(request: Request<Body>, next: Next) -> Response<Body> {
    let session_id = match get_cookie(&request, "idp_session") {
        Some(s) => s,
        None => return next.run(request).await,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let result: Option<(AuthSession, AuthUser)> = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.user_id, u.tenant_id, u.email, u.email_verified
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.id = ?1 AND s.expires_at > ?2",
            )
            .ok()?;
        stmt.query_row(rusqlite::params![session_id, now], |row| {
            let session = AuthSession {
                session_id: row.get(0)?,
                user_id: row.get(1)?,
            };
            let user = AuthUser {
                id: row.get::<_, String>(1)?,
                tenant_id: row.get(2)?,
                email: row.get(3)?,
                email_verified: row.get::<_, i64>(4)? != 0,
            };
            Ok((session, user))
        })
        .ok()
    });

    let mut request = request;
    if let Some((session, user)) = result {
        request.extensions_mut().insert(session);
        request.extensions_mut().insert(user);
    }
    next.run(request).await
}

// ---------------------------------------------------------------------------
// 2. Bearer auth — header-based, for API endpoints
// ---------------------------------------------------------------------------

pub async fn bearer_auth(request: Request<Body>, next: Next) -> Response<Body> {
    let token = match extract_bearer(&request) {
        Some(t) => t,
        None => return unauthorized("Missing or invalid Authorization header"),
    };

    let cfg = config::get();

    // Mode 1: admin token match
    if !cfg.admin_token.is_empty() && constant_time_equal(&token, &cfg.admin_token)
    {
        return next.run(request).await;
    }

    // Mode 2: JWT verification — look up active signing keys and verify
    let user = with_db(|conn| -> Option<AuthUser> {
        let mut stmt = conn
            .prepare("SELECT kid, public_key, alg FROM signing_keys WHERE active = 1")
            .ok()?;
        let keys: Vec<(String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        for (_kid, public_key, _alg) in &keys {
            if let Ok(claims) = verify_token(&token, public_key) {
                // claims is a serde_json::Value with sub, tenant_id, email, etc.
                let sub = claims.get("sub")?.as_str()?.to_string();
                let tenant_id = claims.get("tenant_id")?.as_str()?.to_string();
                let email = claims
                    .get("email")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let email_verified = claims
                    .get("email_verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                return Some(AuthUser {
                    id: sub,
                    tenant_id,
                    email,
                    email_verified,
                });
            }
        }
        None
    });

    match user {
        Some(u) => {
            let mut request = request;
            request.extensions_mut().insert(u);
            next.run(request).await
        }
        None => unauthorized("Invalid or expired token"),
    }
}

// ---------------------------------------------------------------------------
// 3. Admin auth — Bearer token must match IDP_ADMIN_TOKEN exactly
// ---------------------------------------------------------------------------

pub async fn admin_auth(request: Request<Body>, next: Next) -> Response<Body> {
    let token = match extract_bearer(&request) {
        Some(t) => t,
        None => return unauthorized("Missing Authorization header"),
    };

    let cfg = config::get();
    if cfg.admin_token.is_empty() {
        return forbidden("Admin token not configured");
    }

    if constant_time_equal(&token, &cfg.admin_token) {
        next.run(request).await
    } else {
        forbidden("Invalid admin token")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_bearer(request: &Request<Body>) -> Option<String> {
    let header = request.headers().get(header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

fn get_cookie(request: &Request<Body>, name: &str) -> Option<String> {
    let cookies = request.headers().get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

fn unauthorized(msg: &str) -> Response<Body> {
    let body = json!({"error": "unauthorized", "error_description": msg});
    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}

fn forbidden(msg: &str) -> Response<Body> {
    let body = json!({"error": "forbidden", "error_description": msg});
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}
