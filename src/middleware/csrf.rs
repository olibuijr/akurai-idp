use axum::{
    body::Body,
    http::{Method, Request, Response, StatusCode, header},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::json;

use crate::lib::crypto::generate_secure_token;

pub async fn csrf_protection(request: Request<Body>, next: Next) -> Response<Body> {
    let method = request.method().clone();

    // Skip CSRF for safe methods — just ensure cookie exists
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        let has_cookie = get_csrf_cookie(&request).is_some();
        let mut response = next.run(request).await;
        if !has_cookie {
            let token = generate_secure_token(32);
            set_csrf_cookie(&mut response, &token);
        }
        return response;
    }

    // Skip CSRF for Bearer-authenticated requests (API clients)
    if let Some(auth) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(val) = auth.to_str() {
            if val.starts_with("Bearer ") {
                return next.run(request).await;
            }
        }
    }

    // For state-changing methods, verify double-submit cookie
    let cookie_token = match get_csrf_cookie(&request) {
        Some(t) => t,
        None => {
            return csrf_error("Missing CSRF cookie");
        }
    };

    // Check X-CSRF-Token header first, then fall back to _csrf form field
    let submitted_token = get_csrf_header(&request)
        .or_else(|| get_csrf_form_field(&request));

    match submitted_token {
        Some(ref token) if token == &cookie_token => {
            next.run(request).await
        }
        _ => csrf_error("CSRF token mismatch"),
    }
}

fn get_csrf_cookie(request: &Request<Body>) -> Option<String> {
    let cookies = request.headers().get(header::COOKIE)?.to_str().ok()?;
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("_csrf=") {
            return Some(value.to_string());
        }
    }
    None
}

fn get_csrf_header(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get("x-csrf-token")?
        .to_str()
        .ok()
        .map(String::from)
}

fn get_csrf_form_field(request: &Request<Body>) -> Option<String> {
    // We can only read the _csrf field if it was pre-extracted into extensions
    // by an earlier body-parsing layer. For form posts the authorize/token
    // handlers parse the body; this is a best-effort header check.
    request
        .extensions()
        .get::<CsrfFormToken>()
        .map(|t| t.0.clone())
}

/// Handlers that parse form bodies should insert this into extensions
/// before the CSRF middleware runs, or use the X-CSRF-Token header.
#[derive(Clone)]
pub struct CsrfFormToken(pub String);

fn set_csrf_cookie(response: &mut Response<Body>, token: &str) {
    let cookie = format!("_csrf={token}; Path=/; HttpOnly; SameSite=Strict; Secure");
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.parse().unwrap());
}

fn csrf_error(msg: &str) -> Response<Body> {
    let body = json!({
        "error": "csrf_error",
        "error_description": msg
    });
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}
