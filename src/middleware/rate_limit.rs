use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct RateLimitState {
    pub window_ms: u64,
    pub max_requests: u64,
    pub store: Arc<Mutex<HashMap<String, (u64, u64)>>>, // ip -> (count, reset_at_ms)
}

impl RateLimitState {
    pub fn new(window_ms: u64, max_requests: u64) -> Self {
        Self {
            window_ms,
            max_requests,
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Rate-limit middleware. Reads client IP from X-Forwarded-For or falls back to "unknown".
/// Requires `RateLimitState` to be inserted into request extensions (e.g. via Extension layer).
pub async fn rate_limit(request: Request<Body>, next: Next) -> Response<Body> {
    let state = match request.extensions().get::<RateLimitState>().cloned() {
        Some(s) => s,
        None => return next.run(request).await,
    };

    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let current = now_ms();

    let (count, reset_at, remaining) = {
        let mut store = state.store.lock().unwrap();
        let entry = store
            .entry(ip)
            .or_insert((0, current + state.window_ms));

        if current >= entry.1 {
            // Window expired — reset
            entry.0 = 0;
            entry.1 = current + state.window_ms;
        }

        entry.0 += 1;
        let remaining = state.max_requests.saturating_sub(entry.0);
        (entry.0, entry.1, remaining)
    };

    if count > state.max_requests {
        let retry_after = (reset_at.saturating_sub(current) + 999) / 1000;
        let body = json!({
            "error": "too_many_requests",
            "error_description": "Rate limit exceeded. Try again later.",
            "retry_after": retry_after
        });
        let mut resp = (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
        let headers = resp.headers_mut();
        headers.insert(
            "x-ratelimit-limit",
            state.max_requests.to_string().parse().unwrap(),
        );
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert(
            "x-ratelimit-reset",
            (reset_at / 1000).to_string().parse().unwrap(),
        );
        headers.insert(
            "retry-after",
            retry_after.to_string().parse().unwrap(),
        );
        return resp;
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-ratelimit-limit",
        state.max_requests.to_string().parse().unwrap(),
    );
    headers.insert(
        "x-ratelimit-remaining",
        remaining.to_string().parse().unwrap(),
    );
    headers.insert(
        "x-ratelimit-reset",
        (reset_at / 1000).to_string().parse().unwrap(),
    );

    response
}
