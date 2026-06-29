use axum::{
    Json, Router, extract::Request, http::StatusCode, response::IntoResponse, routing::post,
};
use serde::Deserialize;
use serde_json::json;

use crate::db::with_db;
use crate::lib::crypto::sha256;

pub fn router() -> Router {
    Router::new().route("/revoke", post(revoke_endpoint))
}

#[derive(Deserialize, Default)]
struct RevokeRequest {
    token: Option<String>,
    token_type_hint: Option<String>,
    client_id: Option<String>,
    #[allow(dead_code)]
    client_secret: Option<String>,
}

async fn revoke_endpoint(request: Request) -> impl IntoResponse {
    let (basic_id, basic_secret) = extract_basic_auth(&request);

    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 64).await {
        Ok(b) => b,
        Err(_) => return StatusCode::OK.into_response(),
    };

    let mut params: RevokeRequest =
        serde_json::from_slice(&body_bytes).unwrap_or_else(|_| parse_form_urlencoded(&body_bytes));

    if let Some(id) = basic_id {
        params.client_id = Some(id);
    }
    if basic_secret.is_some() {
        params.client_secret = basic_secret;
    }

    // Client auth required
    if params.client_id.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid_client"})),
        )
            .into_response();
    }

    let token = match params.token {
        Some(t) if !t.is_empty() => t,
        _ => return StatusCode::OK.into_response(), // RFC 7009: always 200
    };

    let hint = params.token_type_hint.as_deref();

    // Try as refresh token
    if hint != Some("access_token") {
        let token_hash = sha256(&token);
        let revoked = with_db(|conn| {
            conn.execute(
                "UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ?1",
                rusqlite::params![token_hash],
            )
            .unwrap_or(0)
        });
        if revoked > 0 {
            tracing::info!("Revoked refresh token");
            return StatusCode::OK.into_response();
        }
    }

    // If it's a JWT access token, we can't truly revoke it (stateless),
    // but per RFC 7009 we still return 200
    tracing::info!("Token revocation request for non-revocable token (likely JWT access token)");

    StatusCode::OK.into_response()
}

fn parse_form_urlencoded(bytes: &[u8]) -> RevokeRequest {
    let s = std::str::from_utf8(bytes).unwrap_or("");
    let mut params = RevokeRequest::default();
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
