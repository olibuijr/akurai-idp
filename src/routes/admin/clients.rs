use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::with_db;
use crate::lib::audit;
use crate::lib::crypto::{generate_secure_token, sha256};

#[derive(Deserialize)]
pub struct ListQuery {
    pub tenant_id: Option<String>,
}

#[derive(Serialize)]
pub struct ClientRow {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub first_party: bool,
    pub created_at: i64,
}

#[derive(Deserialize)]
pub struct CreateClient {
    pub name: String,
    pub tenant_id: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Option<Vec<String>>,
    pub scopes: Option<Vec<String>>,
    pub first_party: Option<bool>,
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_clients).post(create_client))
        .route("/{id}", delete(delete_client))
        .route("/{id}/rotate-secret", post(rotate_secret))
}

fn parse_json_array(s: &str) -> Vec<String> {
    crate::lib::parse_json_or_space_separated(s)
}

async fn list_clients(Query(q): Query<ListQuery>) -> impl IntoResponse {
    let result = with_db(|conn| {
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &q.tenant_id {
            Some(tid) => (
                "SELECT id, tenant_id, name, redirect_uris, grant_types, scopes, first_party, created_at FROM clients WHERE tenant_id = ?1".to_string(),
                vec![Box::new(tid.clone()) as Box<dyn rusqlite::types::ToSql>],
            ),
            None => (
                "SELECT id, tenant_id, name, redirect_uris, grant_types, scopes, first_party, created_at FROM clients".to_string(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let redirect_uris_raw: String = row.get(3)?;
            let grant_types_raw: String = row.get(4)?;
            let scopes_raw: String = row.get(5)?;
            Ok(ClientRow {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                name: row.get(2)?,
                redirect_uris: parse_json_array(&redirect_uris_raw),
                grant_types: parse_json_array(&grant_types_raw),
                scopes: parse_json_array(&scopes_raw),
                first_party: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
            })
        })?;
        let mut clients = Vec::new();
        for r in rows {
            clients.push(r?);
        }
        Ok::<_, rusqlite::Error>(clients)
    });

    match result {
        Ok(clients) => Json(json!(clients)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn create_client(Json(body): Json<CreateClient>) -> impl IntoResponse {
    let id = generate_secure_token(16);
    let client_secret = generate_secure_token(40);
    let secret_hash = sha256(&client_secret);
    let now = chrono::Utc::now().timestamp();

    let grant_types = body
        .grant_types
        .unwrap_or_else(|| vec!["authorization_code".to_string()]);
    let scopes = body.scopes.unwrap_or_else(|| {
        vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ]
    });
    let first_party = body.first_party.unwrap_or(false);

    let redirect_uris_json =
        serde_json::to_string(&body.redirect_uris).unwrap_or_else(|_| "[]".to_string());
    let grant_types_json = serde_json::to_string(&grant_types).unwrap_or_else(|_| "[]".to_string());
    let scopes_json = serde_json::to_string(&scopes).unwrap_or_else(|_| "[]".to_string());

    let result = with_db(|conn| {
        conn.execute(
            "INSERT INTO clients (id, tenant_id, name, client_secret_hash, redirect_uris, grant_types, scopes, first_party, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, body.tenant_id, body.name, secret_hash, redirect_uris_json, grant_types_json, scopes_json, first_party as i64, now],
        )
    });

    match result {
        Ok(_) => {
            audit::log_audit(
                Some(&body.tenant_id),
                None,
                audit::CLIENT_CREATED,
                None,
                Some(&json!({"client_id": id, "name": body.name})),
            );
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id,
                    "client_secret": client_secret,
                    "name": body.name,
                    "tenant_id": body.tenant_id,
                    "redirect_uris": body.redirect_uris,
                    "grant_types": grant_types,
                    "scopes": scopes,
                    "first_party": first_party
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_client(Path(id): Path<String>) -> impl IntoResponse {
    let result = with_db(|conn| conn.execute("DELETE FROM clients WHERE id = ?1", [&id]));

    match result {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "client not found"})),
        )
            .into_response(),
        Ok(_) => {
            audit::log_audit(
                None,
                None,
                audit::CLIENT_DELETED,
                None,
                Some(&json!({"client_id": id})),
            );
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn rotate_secret(Path(id): Path<String>) -> impl IntoResponse {
    let new_secret = generate_secure_token(40);
    let new_hash = sha256(&new_secret);

    let result = with_db(|conn| {
        conn.execute(
            "UPDATE clients SET client_secret_hash = ?1 WHERE id = ?2",
            rusqlite::params![new_hash, id],
        )
    });

    match result {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "client not found"})),
        )
            .into_response(),
        Ok(_) => {
            audit::log_audit(
                None,
                None,
                audit::CLIENT_SECRET_ROTATED,
                None,
                Some(&json!({"client_id": id})),
            );
            Json(json!({"id": id, "client_secret": new_secret})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
