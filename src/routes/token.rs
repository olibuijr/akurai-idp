use axum::{
    Json, Router,
    extract::Request,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::post,
};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::db::with_db;
use crate::lib::audit::{self, log_audit_event};
use crate::lib::crypto::{generate_secure_token, sha256, constant_time_equal};
use crate::lib::jwt::sign_token;
use crate::lib::pkce::verify_code_challenge;

pub fn router() -> Router {
    Router::new().route("/token", post(token_endpoint))
}

#[derive(Deserialize, Default)]
struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

async fn token_endpoint(request: Request) -> impl IntoResponse {
    let ip = extract_ip(&request);

    // Extract client credentials from Basic auth header if present
    let (basic_client_id, basic_client_secret) = extract_basic_auth(&request);

    // Parse body as form or JSON
    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 64).await {
        Ok(b) => b,
        Err(_) => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Failed to read body"),
    };

    let mut params: TokenRequest = serde_json::from_slice(&body_bytes)
        .unwrap_or_else(|_| parse_form_urlencoded(&body_bytes));

    // Basic auth overrides body params for client credentials
    if let Some(id) = basic_client_id {
        params.client_id = Some(id);
    }
    if let Some(secret) = basic_client_secret {
        params.client_secret = Some(secret);
    }

    let grant_type = match &params.grant_type {
        Some(gt) => gt.as_str(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing grant_type"),
    };

    match grant_type {
        "authorization_code" => handle_authorization_code(params, &ip).await,
        "refresh_token" => handle_refresh_token(params, &ip).await,
        "client_credentials" => handle_client_credentials(params, &ip).await,
        _ => token_error(StatusCode::BAD_REQUEST, "unsupported_grant_type", "Unsupported grant_type"),
    }
}

async fn handle_authorization_code(params: TokenRequest, ip: &str) -> axum::response::Response {
    let code = match &params.code {
        Some(c) => c.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing code"),
    };
    let client_id = match &params.client_id {
        Some(c) => c.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing client_id"),
    };
    let redirect_uri = match &params.redirect_uri {
        Some(r) => r.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing redirect_uri"),
    };

    let now = now_secs();

    // Look up auth code
    let auth_code = with_db(|conn| {
        conn.query_row(
            "SELECT code, client_id, user_id, scopes, redirect_uri,
                    code_challenge, code_challenge_method, nonce, expires_at, used
             FROM auth_codes WHERE code = ?1",
            rusqlite::params![code],
            |row| {
                Ok(AuthCode {
                    code: row.get(0)?,
                    client_id: row.get(1)?,
                    user_id: row.get(2)?,
                    scopes: row.get(3)?,
                    redirect_uri: row.get(4)?,
                    code_challenge: row.get(5)?,
                    code_challenge_method: row.get(6)?,
                    nonce: row.get(7)?,
                    expires_at: row.get(8)?,
                    used: row.get::<_, i64>(9)? != 0,
                })
            },
        )
        .ok()
    });

    let auth_code = match auth_code {
        Some(ac) => ac,
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Invalid authorization code"),
    };

    // Validate
    if auth_code.used {
        // Code replay — revoke all tokens for this code's user+client
        with_db(|conn| {
            conn.execute(
                "UPDATE refresh_tokens SET revoked = 1 WHERE client_id = ?1 AND user_id = ?2",
                rusqlite::params![auth_code.client_id, auth_code.user_id],
            )
            .ok();
        });
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Authorization code already used");
    }
    if auth_code.expires_at < now {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Authorization code expired");
    }
    if auth_code.client_id != client_id {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Client ID mismatch");
    }
    if auth_code.redirect_uri != redirect_uri {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Redirect URI mismatch");
    }

    // Verify client secret
    if !verify_client_secret(&client_id, params.client_secret.as_deref()) {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client", "Invalid client credentials");
    }

    // PKCE verification
    if let Some(ref challenge) = auth_code.code_challenge {
        let method = auth_code.code_challenge_method.as_deref().unwrap_or("S256");
        let verifier = match &params.code_verifier {
            Some(v) => v,
            None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing code_verifier"),
        };
        if !verify_code_challenge(verifier, challenge, method) {
            return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "PKCE verification failed");
        }
    }

    // Mark code as used
    with_db(|conn| {
        conn.execute("UPDATE auth_codes SET used = 1 WHERE code = ?1", rusqlite::params![code])
            .ok();
    });

    // Load user for token claims
    let user = match load_user(&auth_code.user_id) {
        Some(u) => u,
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "User not found"),
    };

    let scopes = &auth_code.scopes;
    let groups = load_user_groups(&auth_code.user_id);
    let base_url = &config::get().base_url;

    // Build ID token claims
    let id_token_claims = json!({
        "iss": base_url,
        "sub": user.id,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "email": user.email,
        "email_verified": user.email_verified,
        "tenant_id": user.tenant_id,
        "groups": groups,
        "nonce": auth_code.nonce,
    });

    // Build access token claims
    let access_token_claims = json!({
        "iss": base_url,
        "sub": user.id,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "scope": scopes,
        "tenant_id": user.tenant_id,
        "email": user.email,
        "email_verified": user.email_verified,
        "groups": groups,
    });

    let (kid, private_key) = match get_active_signing_key() {
        Some(k) => k,
        None => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "No signing key available"),
    };

    let access_token = match sign_token(&access_token_claims, &private_key, &kid, 3600) {
        Ok(t) => t,
        Err(_) => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Failed to sign token"),
    };

    let id_token = match sign_token(&id_token_claims, &private_key, &kid, 3600) {
        Ok(t) => t,
        Err(_) => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Failed to sign token"),
    };

    // Issue refresh token if offline_access scope
    let refresh_token = if scopes.contains("offline_access") {
        let rt = generate_secure_token(32);
        let rt_hash = sha256(&rt);
        let expires = now + 30 * 24 * 3600; // 30 days
        with_db(|conn| {
            conn.execute(
                "INSERT INTO refresh_tokens (token_hash, client_id, user_id, scopes, expires_at, revoked)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                rusqlite::params![rt_hash, client_id, user.id, scopes, expires],
            )
            .ok();
        });
        Some(rt)
    } else {
        None
    };

    // Audit
    let audit_meta = json!({"grant_type": "authorization_code", "client_id": client_id});
    with_db(|conn| {
        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.id),
            audit::TOKEN_ISSUED,
            Some(ip),
            Some(&audit_meta),
        );
    });

    let mut response = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 3600,
        "id_token": id_token,
        "scope": scopes,
    });
    if let Some(rt) = refresh_token {
        response["refresh_token"] = json!(rt);
    }

    (StatusCode::OK, Json(response)).into_response()
}

async fn handle_refresh_token(params: TokenRequest, ip: &str) -> axum::response::Response {
    let rt = match &params.refresh_token {
        Some(t) => t.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing refresh_token"),
    };
    let client_id = match &params.client_id {
        Some(c) => c.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing client_id"),
    };

    if !verify_client_secret(&client_id, params.client_secret.as_deref()) {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client", "Invalid client credentials");
    }

    let rt_hash = sha256(&rt);
    let now = now_secs();

    // Look up refresh token
    let stored = with_db(|conn| {
        conn.query_row(
            "SELECT token_hash, client_id, user_id, scopes, expires_at, revoked
             FROM refresh_tokens WHERE token_hash = ?1",
            rusqlite::params![rt_hash],
            |row| {
                Ok(StoredRefreshToken {
                    token_hash: row.get(0)?,
                    client_id: row.get(1)?,
                    user_id: row.get(2)?,
                    scopes: row.get(3)?,
                    expires_at: row.get(4)?,
                    revoked: row.get::<_, i64>(5)? != 0,
                })
            },
        )
        .ok()
    });

    let stored = match stored {
        Some(s) => s,
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Invalid refresh token"),
    };

    if stored.revoked {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Refresh token revoked");
    }
    if stored.expires_at < now {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Refresh token expired");
    }
    if stored.client_id != client_id {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "Client ID mismatch");
    }

    // Rotate: revoke old, issue new
    let new_rt = generate_secure_token(32);
    let new_rt_hash = sha256(&new_rt);
    let new_expires = now + 30 * 24 * 3600;

    with_db(|conn| {
        conn.execute(
            "UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ?1",
            rusqlite::params![stored.token_hash],
        )
        .ok();
        conn.execute(
            "INSERT INTO refresh_tokens (token_hash, client_id, user_id, scopes, expires_at, revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            rusqlite::params![new_rt_hash, client_id, stored.user_id, stored.scopes, new_expires],
        )
        .ok();
    });

    let user = match load_user(&stored.user_id) {
        Some(u) => u,
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "User not found"),
    };

    let groups = load_user_groups(&stored.user_id);
    let base_url = &config::get().base_url;

    let access_token_claims = json!({
        "iss": base_url,
        "sub": user.id,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "scope": stored.scopes,
        "tenant_id": user.tenant_id,
        "email": user.email,
        "email_verified": user.email_verified,
        "groups": groups,
    });

    let id_token_claims = json!({
        "iss": base_url,
        "sub": user.id,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "email": user.email,
        "email_verified": user.email_verified,
        "tenant_id": user.tenant_id,
        "groups": groups,
    });

    let (kid, private_key) = match get_active_signing_key() {
        Some(k) => k,
        None => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "No signing key"),
    };

    let access_token = match sign_token(&access_token_claims, &private_key, &kid, 3600) {
        Ok(t) => t,
        Err(_) => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Sign failed"),
    };
    let id_token = match sign_token(&id_token_claims, &private_key, &kid, 3600) {
        Ok(t) => t,
        Err(_) => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Sign failed"),
    };

    let audit_meta = json!({"grant_type": "refresh_token", "client_id": client_id});
    with_db(|conn| {
        log_audit_event(
            conn,
            Some(&user.tenant_id),
            Some(&user.id),
            audit::TOKEN_ISSUED,
            Some(ip),
            Some(&audit_meta),
        );
    });

    let response = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": new_rt,
        "id_token": id_token,
        "scope": stored.scopes,
    });

    (StatusCode::OK, Json(response)).into_response()
}

async fn handle_client_credentials(params: TokenRequest, ip: &str) -> axum::response::Response {
    let client_id = match &params.client_id {
        Some(c) => c.clone(),
        None => return token_error(StatusCode::BAD_REQUEST, "invalid_request", "Missing client_id"),
    };

    if !verify_client_secret(&client_id, params.client_secret.as_deref()) {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client", "Invalid client credentials");
    }

    // Verify client supports client_credentials grant
    let client = with_db(|conn| {
        conn.query_row(
            "SELECT id, tenant_id, grant_types, scopes FROM clients WHERE id = ?1",
            rusqlite::params![client_id],
            |row| {
                Ok(ClientInfo {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    grant_types: row.get(2)?,
                    scopes: row.get(3)?,
                })
            },
        )
        .ok()
    });

    let client = match client {
        Some(c) => c,
        None => return token_error(StatusCode::UNAUTHORIZED, "invalid_client", "Client not found"),
    };

    let grant_types = crate::lib::parse_json_or_space_separated(&client.grant_types);
    if !grant_types.iter().any(|g| g == "client_credentials") {
        return token_error(StatusCode::BAD_REQUEST, "unauthorized_client", "Grant type not allowed");
    }

    let requested_scope = params.scope.as_deref().unwrap_or(&client.scopes);
    let now = now_secs();
    let base_url = &config::get().base_url;

    let access_token_claims = json!({
        "iss": base_url,
        "sub": client_id,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "scope": requested_scope,
        "tenant_id": client.tenant_id,
    });

    let (kid, private_key) = match get_active_signing_key() {
        Some(k) => k,
        None => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "No signing key"),
    };

    let access_token = match sign_token(&access_token_claims, &private_key, &kid, 3600) {
        Ok(t) => t,
        Err(_) => return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Sign failed"),
    };

    let audit_meta = json!({"grant_type": "client_credentials", "client_id": client_id});
    with_db(|conn| {
        log_audit_event(
            conn,
            Some(&client.tenant_id),
            None,
            audit::TOKEN_ISSUED,
            Some(ip),
            Some(&audit_meta),
        );
    });

    let response = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": requested_scope,
    });

    (StatusCode::OK, Json(response)).into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct AuthCode {
    code: String,
    client_id: String,
    user_id: String,
    scopes: String,
    redirect_uri: String,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    nonce: Option<String>,
    expires_at: i64,
    used: bool,
}

struct StoredRefreshToken {
    token_hash: String,
    client_id: String,
    user_id: String,
    scopes: String,
    expires_at: i64,
    revoked: bool,
}

#[allow(dead_code)]
struct ClientInfo {
    id: String,
    tenant_id: String,
    grant_types: String,
    scopes: String,
}

struct UserInfo {
    id: String,
    tenant_id: String,
    email: String,
    email_verified: bool,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn load_user(user_id: &str) -> Option<UserInfo> {
    with_db(|conn| {
        conn.query_row(
            "SELECT id, tenant_id, email, email_verified FROM users WHERE id = ?1",
            rusqlite::params![user_id],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    email: row.get(2)?,
                    email_verified: row.get::<_, i64>(3)? != 0,
                })
            },
        )
        .ok()
    })
}

fn load_user_groups(user_id: &str) -> Vec<String> {
    with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT g.name FROM groups g
                 JOIN user_groups ug ON ug.group_id = g.id
                 WHERE ug.user_id = ?1",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![user_id], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    })
}

fn verify_client_secret(client_id: &str, secret: Option<&str>) -> bool {
    let secret = match secret {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };

    let stored_hash: Option<String> = with_db(|conn| {
        conn.query_row(
            "SELECT client_secret_hash FROM clients WHERE id = ?1",
            rusqlite::params![client_id],
            |row| row.get(0),
        )
        .ok()
    });

    match stored_hash {
        Some(hash) => {
            let secret_hash = sha256(secret);
            constant_time_equal(&secret_hash, &hash)
        }
        None => false,
    }
}

fn get_active_signing_key() -> Option<(String, String)> {
    with_db(|conn| {
        conn.query_row(
            "SELECT kid, private_key_enc FROM signing_keys WHERE active = 1 ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()
    })
}

fn extract_basic_auth(request: &Request) -> (Option<String>, Option<String>) {
    let header = match request.headers().get(header::AUTHORIZATION) {
        Some(h) => h,
        None => return (None, None),
    };
    let value = match header.to_str().ok() {
        Some(v) => v,
        None => return (None, None),
    };
    let encoded = match value.strip_prefix("Basic ") {
        Some(e) => e,
        None => return (None, None),
    };
    use base64::Engine;
    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(d) => d,
        Err(_) => return (None, None),
    };
    let decoded_str = match String::from_utf8(decoded) {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    match decoded_str.split_once(':') {
        Some((id, secret)) => (Some(id.to_string()), Some(secret.to_string())),
        None => (None, None),
    }
}

fn extract_ip(request: &Request) -> String {
    // Try X-Forwarded-For first, then peer addr
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn parse_form_urlencoded(bytes: &[u8]) -> TokenRequest {
    let s = std::str::from_utf8(bytes).unwrap_or("");
    let mut params = TokenRequest::default();
    for pair in s.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = urldecode(value);
            match key {
                "grant_type" => params.grant_type = Some(value),
                "code" => params.code = Some(value),
                "redirect_uri" => params.redirect_uri = Some(value),
                "client_id" => params.client_id = Some(value),
                "client_secret" => params.client_secret = Some(value),
                "code_verifier" => params.code_verifier = Some(value),
                "refresh_token" => params.refresh_token = Some(value),
                "scope" => params.scope = Some(value),
                _ => {}
            }
        }
    }
    params
}

fn urldecode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut result = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let hex = [hi, lo];
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&hex).unwrap_or("00"), 16) {
                result.push(val);
            }
        } else {
            result.push(b);
        }
    }
    String::from_utf8(result).unwrap_or_default()
}

fn token_error(status: StatusCode, error: &str, description: &str) -> axum::response::Response {
    let body = json!({
        "error": error,
        "error_description": description,
    });
    (status, Json(body)).into_response()
}
