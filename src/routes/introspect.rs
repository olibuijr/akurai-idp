use axum::{
    Json, Router, extract::Request, http::StatusCode, response::IntoResponse, routing::post,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::db::with_db;
use crate::lib::crypto::sha256;
use crate::lib::jwt::verify_token;

pub fn router() -> Router {
    Router::new().route("/introspect", post(introspect_endpoint))
}

#[derive(Deserialize, Default)]
struct IntrospectRequest {
    token: Option<String>,
    token_type_hint: Option<String>,
    client_id: Option<String>,
    #[allow(dead_code)]
    client_secret: Option<String>,
}

async fn introspect_endpoint(request: Request) -> impl IntoResponse {
    let (basic_id, basic_secret) = extract_basic_auth(&request);

    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 64).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::OK, Json(json!({"active": false}))).into_response(),
    };

    let mut params: IntrospectRequest =
        serde_json::from_slice(&body_bytes).unwrap_or_else(|_| parse_form_urlencoded(&body_bytes));

    // Basic auth overrides body params
    if let Some(id) = basic_id {
        params.client_id = Some(id);
    }
    if basic_secret.is_some() {
        params.client_secret = basic_secret;
    }

    if params.client_id.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid_client"})),
        )
            .into_response();
    }

    let token = match params.token {
        Some(t) if !t.is_empty() => t,
        _ => return (StatusCode::OK, Json(json!({"active": false}))).into_response(),
    };

    let hint = params.token_type_hint.as_deref();

    // Try refresh token first (if hinted or no hint)
    if hint != Some("access_token") {
        let token_hash = sha256(&token);
        let result = with_db(|conn| {
            conn.query_row(
                "SELECT rt.token_hash, rt.client_id, rt.user_id, rt.scopes, rt.expires_at, rt.revoked,
                        u.email, u.tenant_id
                 FROM refresh_tokens rt
                 JOIN users u ON u.id = rt.user_id
                 WHERE rt.token_hash = ?1",
                rusqlite::params![token_hash],
                |row| {
                    let expires_at: i64 = row.get(4)?;
                    let revoked: bool = row.get::<_, i64>(5)? != 0;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let active = !revoked && expires_at > now;
                    Ok(json!({
                        "active": active,
                        "token_type": "refresh_token",
                        "client_id": row.get::<_, String>(1)?,
                        "sub": row.get::<_, String>(2)?,
                        "scope": row.get::<_, String>(3)?,
                        "exp": expires_at,
                        "email": row.get::<_, String>(6)?,
                        "tenant_id": row.get::<_, String>(7)?,
                    }))
                },
            )
            .ok()
        });

        if let Some(resp) = result {
            return (StatusCode::OK, Json(resp)).into_response();
        }
    }

    // Try as JWT access token
    let jwt_result = with_db(|conn| -> Option<Value> {
        let mut stmt = conn
            .prepare("SELECT kid, public_key, alg FROM signing_keys WHERE active = 1")
            .ok()?;
        let keys: Vec<(String, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        for (_kid, public_key, _alg) in &keys {
            if let Ok(claims) = verify_token(&token, public_key) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let exp = claims.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
                let active = exp > now;
                let mut result = json!({"active": active, "token_type": "access_token"});
                if let Some(obj) = claims.as_object() {
                    for (k, v) in obj {
                        result[k] = v.clone();
                    }
                }
                result["active"] = json!(active);
                return Some(result);
            }
        }
        None
    });

    if let Some(resp) = jwt_result {
        return (StatusCode::OK, Json(resp)).into_response();
    }

    (StatusCode::OK, Json(json!({"active": false}))).into_response()
}

fn parse_form_urlencoded(bytes: &[u8]) -> IntrospectRequest {
    let s = std::str::from_utf8(bytes).unwrap_or("");
    let mut params = IntrospectRequest::default();
    for pair in s.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = urldecode(value);
            match key {
                "token" => params.token = Some(value),
                "token_type_hint" => params.token_type_hint = Some(value),
                "client_id" => params.client_id = Some(value),
                "client_secret" => params.client_secret = Some(value),
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

fn extract_basic_auth(request: &Request) -> (Option<String>, Option<String>) {
    let header = match request.headers().get(axum::http::header::AUTHORIZATION) {
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
