use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::db::with_db;
use crate::lib::audit::{self, log_audit_event};
use crate::lib::crypto::generate_secure_token;
use crate::middleware::auth::{AuthSession, AuthUser};

pub fn router() -> Router {
    Router::new().route("/authorize", get(authorize_endpoint))
}

#[derive(Deserialize)]
struct AuthorizeParams {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    nonce: Option<String>,
}

async fn authorize_endpoint(
    Query(params): Query<AuthorizeParams>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    // Validate required params
    let client_id = match &params.client_id {
        Some(c) if !c.is_empty() => c.clone(),
        _ => return auth_error_redirect(None, params.state.as_deref(), "invalid_request", "Missing client_id"),
    };

    let redirect_uri = match &params.redirect_uri {
        Some(r) if !r.is_empty() => r.clone(),
        _ => return auth_error_page("Missing redirect_uri"),
    };

    let response_type = params.response_type.as_deref().unwrap_or("");
    if response_type != "code" {
        return auth_error_redirect(
            Some(&redirect_uri),
            params.state.as_deref(),
            "unsupported_response_type",
            "Only response_type=code is supported",
        );
    }

    // Look up client
    let client = with_db(|conn| {
        conn.query_row(
            "SELECT id, tenant_id, redirect_uris, scopes, first_party FROM clients WHERE id = ?1",
            rusqlite::params![client_id],
            |row| {
                Ok(ClientRow {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    redirect_uris: row.get(2)?,
                    scopes: row.get(3)?,
                    first_party: row.get::<_, i64>(4)? != 0,
                })
            },
        )
        .ok()
    });

    let client = match client {
        Some(c) => c,
        None => return auth_error_page("Unknown client_id"),
    };

    // Validate redirect_uri against registered URIs
    let allowed_uris: Vec<&str> = client.redirect_uris.split_whitespace().collect();
    if !allowed_uris.contains(&redirect_uri.as_str()) {
        return auth_error_page("Invalid redirect_uri");
    }

    // Validate scopes
    let requested_scopes = params.scope.as_deref().unwrap_or("openid");
    let allowed_scopes: Vec<&str> = client.scopes.split_whitespace().collect();
    for scope in requested_scopes.split_whitespace() {
        if !allowed_scopes.contains(&scope) {
            return auth_error_redirect(
                Some(&redirect_uri),
                params.state.as_deref(),
                "invalid_scope",
                &format!("Scope '{scope}' not allowed for this client"),
            );
        }
    }

    // PKCE validation (require S256 if code_challenge provided)
    if let Some(ref method) = params.code_challenge_method {
        if method != "S256" {
            return auth_error_redirect(
                Some(&redirect_uri),
                params.state.as_deref(),
                "invalid_request",
                "Only S256 code_challenge_method is supported",
            );
        }
    }
    if params.code_challenge.is_some() && params.code_challenge_method.is_none() {
        return auth_error_redirect(
            Some(&redirect_uri),
            params.state.as_deref(),
            "invalid_request",
            "code_challenge_method required when code_challenge is present",
        );
    }

    // Check session — user must be logged in
    let user = request.extensions().get::<AuthUser>().cloned();
    let _session = request.extensions().get::<AuthSession>().cloned();

    let user = match user {
        Some(u) => u,
        None => {
            // Redirect to login with return URL
            let base = &config::get().base_url;
            let return_url = format!(
                "{base}/authorize?client_id={}&redirect_uri={}&response_type={}&scope={}&state={}&nonce={}{}",
                urlencoded(&client_id),
                urlencoded(&redirect_uri),
                urlencoded(response_type),
                urlencoded(requested_scopes),
                urlencoded(params.state.as_deref().unwrap_or("")),
                urlencoded(params.nonce.as_deref().unwrap_or("")),
                params.code_challenge.as_ref().map(|c|
                    format!("&code_challenge={}&code_challenge_method={}",
                        urlencoded(c),
                        urlencoded(params.code_challenge_method.as_deref().unwrap_or("S256")))
                ).unwrap_or_default(),
            );
            return Redirect::to(&format!("/login?return_to={}", urlencoded(&return_url))).into_response();
        }
    };

    // Tenant isolation: user's tenant must match client's tenant
    if user.tenant_id != client.tenant_id {
        return auth_error_redirect(
            Some(&redirect_uri),
            params.state.as_deref(),
            "access_denied",
            "User does not belong to client tenant",
        );
    }

    // First-party clients: auto-approve (no consent screen)
    if client.first_party {
        return issue_auth_code(
            &client_id,
            &user.id,
            &user.tenant_id,
            requested_scopes,
            &redirect_uri,
            params.code_challenge.as_deref(),
            params.code_challenge_method.as_deref(),
            params.nonce.as_deref(),
            params.state.as_deref(),
        );
    }

    // Third-party clients: consent page placeholder
    // In production, render a consent form and only issue code after user approves.
    // For now, return a simple HTML consent page.
    let html = format!(
        r#"<!DOCTYPE html>
<html><head><title>Authorize {name}</title></head>
<body>
<h1>Authorize {name}</h1>
<p>This application is requesting access to your account with scopes: {scopes}</p>
<p>Consent flow not yet implemented. First-party clients are auto-approved.</p>
</body></html>"#,
        name = client_id,
        scopes = requested_scopes,
    );
    (StatusCode::OK, [("content-type", "text/html")], html).into_response()
}

fn issue_auth_code(
    client_id: &str,
    user_id: &str,
    tenant_id: &str,
    scopes: &str,
    redirect_uri: &str,
    code_challenge: Option<&str>,
    code_challenge_method: Option<&str>,
    nonce: Option<&str>,
    state: Option<&str>,
) -> axum::response::Response {
    let code = generate_secure_token(32);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let expires_at = now + 600; // 10 minutes

    with_db(|conn| {
        conn.execute(
            "INSERT INTO auth_codes (code, client_id, user_id, scopes, redirect_uri,
                                     code_challenge, code_challenge_method, nonce, expires_at, used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
            rusqlite::params![
                code,
                client_id,
                user_id,
                scopes,
                redirect_uri,
                code_challenge,
                code_challenge_method,
                nonce,
                expires_at,
            ],
        )
        .expect("failed to insert auth code");

        let audit_meta = json!({"client_id": client_id});
        log_audit_event(
            conn,
            Some(tenant_id),
            Some(user_id),
            audit::AUTH_CODE_ISSUED,
            None,
            Some(&audit_meta),
        );
    });

    let mut location = format!("{redirect_uri}?code={code}");
    if let Some(s) = state {
        location.push_str(&format!("&state={}", urlencoded(s)));
    }

    Redirect::to(&location).into_response()
}

fn auth_error_redirect(
    redirect_uri: Option<&str>,
    state: Option<&str>,
    error: &str,
    description: &str,
) -> axum::response::Response {
    match redirect_uri {
        Some(uri) => {
            let mut location = format!("{uri}?error={error}&error_description={}", urlencoded(description));
            if let Some(s) = state {
                location.push_str(&format!("&state={}", urlencoded(s)));
            }
            Redirect::to(&location).into_response()
        }
        None => auth_error_page(description),
    }
}

fn auth_error_page(description: &str) -> axum::response::Response {
    let html = format!(
        r#"<!DOCTYPE html>
<html><head><title>Authorization Error</title></head>
<body><h1>Authorization Error</h1><p>{description}</p></body></html>"#,
    );
    (StatusCode::BAD_REQUEST, [("content-type", "text/html")], html).into_response()
}

fn urlencoded(s: &str) -> String {
    // Percent-encode for URL query parameters
    let mut encoded = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

#[allow(dead_code)]
struct ClientRow {
    id: String,
    tenant_id: String,
    redirect_uris: String,
    scopes: String,
    first_party: bool,
}
