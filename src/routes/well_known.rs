use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

use crate::config;
use crate::db::with_db;
use crate::lib::jwt::export_public_key_jwk;

pub fn router() -> Router {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(openid_configuration),
        )
        .route("/jwks", get(jwks))
}

async fn openid_configuration() -> Json<Value> {
    let base = &config::get().base_url;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "userinfo_endpoint": format!("{base}/userinfo"),
        "jwks_uri": format!("{base}/jwks"),
        "introspection_endpoint": format!("{base}/introspect"),
        "revocation_endpoint": format!("{base}/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "scopes_supported": ["openid", "profile", "email", "groups", "offline_access"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "email", "email_verified", "groups", "tenant_id"],
        "code_challenge_methods_supported": ["S256"],
    }))
}

async fn jwks() -> Json<Value> {
    let keys: Vec<Value> = with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT kid, public_key, alg FROM signing_keys WHERE active = 1")
            .expect("failed to prepare signing_keys query");
        stmt.query_map([], |row| {
            let kid: String = row.get(0)?;
            let public_key: String = row.get(1)?;
            let alg: String = row.get(2)?;
            Ok((kid, public_key, alg))
        })
        .expect("failed to query signing_keys")
        .filter_map(|r| r.ok())
        .filter_map(|(kid, public_key, _alg)| export_public_key_jwk(&public_key, &kid).ok())
        .collect()
    });

    Json(json!({ "keys": keys }))
}
