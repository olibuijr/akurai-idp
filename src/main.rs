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
    Json, Router,
};
use serde_json::json;
use subtle::ConstantTimeEq;

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

    // Public OIDC routes
    let oidc_routes = Router::new()
        .route("/.well-known/openid-configuration", get(routes::well_known::openid_configuration))
        .route("/.well-known/jwks.json", get(routes::well_known::jwks))
        .route("/authorize", get(routes::authorize::authorize))
        .route("/token", axum::routing::post(routes::token::token))
        .route("/userinfo", get(routes::userinfo::userinfo).post(routes::userinfo::userinfo))
        .route("/introspect", axum::routing::post(routes::introspect::introspect))
        .route("/revoke", axum::routing::post(routes::revoke::revoke));

    // Auth page routes (login, mfa, logout)
    let auth_pages = Router::new()
        .route("/login", get(routes::auth_pages::login_page).post(routes::auth_pages::login_submit))
        .route("/mfa", get(routes::auth_pages::mfa_page).post(routes::auth_pages::mfa_submit))
        .route("/logout", get(routes::auth_pages::logout).post(routes::auth_pages::logout));

    // Account routes (session-auth'd)
    let account_routes = Router::new()
        .nest("/account", routes::account::router());

    // Apply CSRF middleware to auth pages
    let auth_pages_with_csrf = auth_pages
        .layer(axum_mw::from_fn(middleware::csrf::csrf_middleware));

    // Apply tighter rate limits to sensitive endpoints
    let token_route = Router::new()
        .route("/token", axum::routing::post(routes::token::token))
        .layer(axum_mw::from_fn(|req, next: axum::middleware::Next| {
            middleware::rate_limit::rate_limit_middleware(req, next, 30, 60)
        }));

    let introspect_route = Router::new()
        .route("/introspect", axum::routing::post(routes::introspect::introspect))
        .layer(axum_mw::from_fn(|req, next: axum::middleware::Next| {
            middleware::rate_limit::rate_limit_middleware(req, next, 60, 60)
        }));

    let revoke_route = Router::new()
        .route("/revoke", axum::routing::post(routes::revoke::revoke))
        .layer(axum_mw::from_fn(|req, next: axum::middleware::Next| {
            middleware::rate_limit::rate_limit_middleware(req, next, 30, 60)
        }));

    let login_rate_limited = Router::new()
        .route("/login", get(routes::auth_pages::login_page).post(routes::auth_pages::login_submit))
        .layer(axum_mw::from_fn(middleware::csrf::csrf_middleware))
        .layer(axum_mw::from_fn(|req, next: axum::middleware::Next| {
            middleware::rate_limit::rate_limit_middleware(req, next, 20, 60)
        }));

    // Build the full app with route-specific rate limits taking precedence
    let app = Router::new()
        // Tighter-rate-limited routes first (more specific)
        .merge(login_rate_limited)
        .merge(token_route)
        .merge(introspect_route)
        .merge(revoke_route)
        // MFA + logout with CSRF
        .route("/mfa", get(routes::auth_pages::mfa_page).post(routes::auth_pages::mfa_submit))
        .route("/logout", get(routes::auth_pages::logout).post(routes::auth_pages::logout))
        // OIDC discovery + authorize + userinfo (standard rate limit)
        .route("/.well-known/openid-configuration", get(routes::well_known::openid_configuration))
        .route("/.well-known/jwks.json", get(routes::well_known::jwks))
        .route("/authorize", get(routes::authorize::authorize))
        .route("/userinfo", get(routes::userinfo::userinfo).post(routes::userinfo::userinfo))
        // Account (session-auth'd)
        .merge(account_routes)
        // Admin API
        .nest("/admin", admin_router)
        // Health check
        .route("/health", get(health))
        // 404 catch-all
        .fallback(not_found)
        // Global middleware: secure headers + default rate limit
        .layer(axum_mw::from_fn(middleware::secure_headers::secure_headers_middleware))
        .layer(axum_mw::from_fn(|req, next: axum::middleware::Next| {
            middleware::rate_limit::rate_limit_middleware(req, next, 100, 60)
        }));

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
