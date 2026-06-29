use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, Response, StatusCode, header},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::json;

use crate::lib::crypto::generate_secure_token;

pub async fn csrf_protection(mut request: Request<Body>, next: Next) -> Response<Body> {
    let method = request.method().clone();

    // Skip CSRF for safe methods — just ensure cookie exists
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        let generated_token = if get_csrf_cookie(&request).is_some() {
            None
        } else {
            let token = generate_secure_token(32);
            set_csrf_request_cookie(&mut request, &token);
            Some(token)
        };
        let mut response = next.run(request).await;
        if let Some(token) = generated_token {
            set_csrf_cookie(&mut response, &token);
        }
        return response;
    }

    // Skip CSRF for Bearer-authenticated requests (API clients)
    if let Some(auth) = request.headers().get(header::AUTHORIZATION)
        && let Ok(val) = auth.to_str()
        && val.starts_with("Bearer ")
    {
        return next.run(request).await;
    }

    // For state-changing methods, verify double-submit cookie
    let cookie_token = match get_csrf_cookie(&request) {
        Some(t) => t,
        None => {
            return csrf_error("Missing CSRF cookie");
        }
    };

    // Check X-CSRF-Token header first
    if let Some(ref token) = get_csrf_header(&request) {
        if token == &cookie_token {
            return next.run(request).await;
        }
        return csrf_error("CSRF token mismatch");
    }

    // For form-urlencoded POSTs, extract the body to read _csrf field
    let is_form = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);

    if is_form {
        // Consume the body to extract _csrf field
        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, 1024 * 64).await {
            Ok(b) => b,
            Err(_) => return csrf_error("Failed to read request body"),
        };

        let body_str = String::from_utf8_lossy(&bytes);
        let form_token = form_urlencoded::parse(body_str.as_bytes())
            .find(|(k, _)| k == "_csrf")
            .map(|(_, v)| v.into_owned());

        // Reconstruct the request with the consumed body
        let request = Request::from_parts(parts, Body::from(bytes));

        match form_token {
            Some(ref token) if token == &cookie_token => next.run(request).await,
            _ => csrf_error("CSRF token mismatch"),
        }
    } else {
        csrf_error("CSRF token mismatch")
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

fn set_csrf_request_cookie(request: &mut Request<Body>, token: &str) {
    let value = match request
        .headers()
        .get(header::COOKIE)
        .and_then(|header| header.to_str().ok())
    {
        Some(existing) if !existing.trim().is_empty() => format!("{existing}; _csrf={token}"),
        _ => format!("_csrf={token}"),
    };

    if let Ok(value) = HeaderValue::from_str(&value) {
        request.headers_mut().insert(header::COOKIE, value);
    }
}

fn get_csrf_header(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get("x-csrf-token")?
        .to_str()
        .ok()
        .map(String::from)
}

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
