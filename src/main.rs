pub mod config;
pub mod db;
pub mod lib;
pub mod middleware;
pub mod routes;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware as axum_mw,
    response::IntoResponse,
    routing::get,
    Extension, Json, Router,
};
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::middleware::rate_limit::RateLimitState;

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("akurai-idp starting up");

    // Ensure DB is initialized (triggers LazyLock)
    db::with_db(|_conn| {});

    // Ensure signing key exists
    ensure_signing_key();

    let cfg = config::get();

    // --- Build the router ---

    // Admin routes (token-auth'd)
    let admin_router = Router::new()
        .nest("/users", routes::admin::users::router())
        .nest("/groups", routes::admin::groups::router())
        .nest("/tenants", routes::admin::tenants::router())
        .nest("/clients", routes::admin::clients::router())
        .nest("/audit", routes::admin::audit::router())
        .layer(axum_mw::from_fn(admin_auth_middleware));

    // Build the full app using each module's router()
    let app = Router::new()
        // Auth pages (login/mfa/logout) with CSRF
        .merge(
            routes::auth_pages::router()
                .layer(axum_mw::from_fn(middleware::csrf::csrf_protection))
        )
        // OIDC discovery
        .merge(routes::well_known::router())
        // OIDC endpoints
        .merge(
            routes::authorize::router()
                .layer(axum_mw::from_fn(middleware::auth::session_optional))
        )
        .merge(routes::token::router())
        .merge(
            routes::userinfo::router().layer(axum_mw::from_fn(middleware::auth::bearer_auth)),
        )
        .merge(routes::introspect::router())
        .merge(routes::revoke::router())
        // Account (session-auth'd)
        .nest("/account", routes::account::router())
        // Admin API
        .nest("/admin", admin_router)
        // Health check
        .route("/health", get(health))
        // 404 catch-all
        .fallback(not_found)
        // Global middleware: secure headers + default rate limit
        .layer(axum_mw::from_fn(middleware::secure_headers::secure_headers))
        .layer(axum_mw::from_fn(middleware::rate_limit::rate_limit))
        .layer(Extension(RateLimitState::new(60_000, 100)));

    tracing::info!("listening on {}", cfg.listen_addr);

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr)
        .await
        .expect("failed to bind listener");

    axum::serve(listener, app)
        .await
        .expect("server error");
}

/// Admin auth middleware: requires Bearer token matching IDP_ADMIN_TOKEN (constant-time compare).
async fn admin_auth_middleware(
    req: Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let cfg = config::get();

    if cfg.admin_token.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "admin API not configured (IDP_ADMIN_TOKEN not set)"})),
        )
            .into_response();
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing or invalid Authorization header"})),
            )
                .into_response();
        }
    };

    // Constant-time comparison
    let expected = cfg.admin_token.as_bytes();
    let provided = token.as_bytes();

    if expected.len() != provided.len() || expected.ct_eq(provided).unwrap_u8() != 1 {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid admin token"})),
        )
            .into_response();
    }

    next.run(req).await.into_response()
}

/// Ensure at least one active signing key exists. Generate one if the table is empty.
fn ensure_signing_key() {
    let has_active = db::with_db(|conn| {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM signing_keys WHERE active = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        count > 0
    });

    if !has_active {
        tracing::info!("no active signing key found, generating one");
        let (kid, pub_pem, priv_pem) = lib::jwt::generate_signing_key();
        let now = chrono::Utc::now().timestamp();

        db::with_db(|conn| {
            conn.execute(
                "INSERT INTO signing_keys (kid, alg, public_key, private_key_enc, active, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
                rusqlite::params![kid, "EdDSA", pub_pem, priv_pem, now],
            )
            .expect("failed to insert signing key");
        });

        tracing::info!("signing key generated: kid={kid}");
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({"ok": true, "service": "akurai-idp"}))
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({"error": "not found"})))
}
