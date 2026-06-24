use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::with_db;
use crate::lib::audit;
use crate::lib::crypto::generate_secure_token;

#[derive(Deserialize)]
pub struct ListQuery {
    pub tenant_id: Option<String>,
}

#[derive(Serialize)]
pub struct GroupRow {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub tenant_id: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct AddMember {
    pub user_id: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_groups).post(create_group))
        .route("/{id}", delete(delete_group))
        .route("/{id}/members", post(add_member))
        .route("/{id}/members/{userId}", delete(remove_member))
}

async fn list_groups(Query(q): Query<ListQuery>) -> impl IntoResponse {
    let result = with_db(|conn| {
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &q.tenant_id {
            Some(tid) => (
                "SELECT id, tenant_id, name, description FROM groups_ WHERE tenant_id = ?1".to_string(),
                vec![Box::new(tid.clone()) as Box<dyn rusqlite::types::ToSql>],
            ),
            None => (
                "SELECT id, tenant_id, name, description FROM groups_".to_string(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(GroupRow {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
            })
        })?;
        let mut groups = Vec::new();
        for r in rows {
            groups.push(r?);
        }
        Ok::<_, rusqlite::Error>(groups)
    });

    match result {
        Ok(groups) => Json(json!(groups)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn create_group(Json(body): Json<CreateGroup>) -> impl IntoResponse {
    let id = generate_secure_token(16);

    let result = with_db(|conn| {
        conn.execute(
            "INSERT INTO groups_ (id, tenant_id, name, description) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, body.tenant_id, body.name, body.description],
        )
    });

    match result {
        Ok(_) => {
            audit::log_audit(
                Some(&body.tenant_id),
                None,
                audit::GROUP_CREATED,
                None,
                Some(&json!({"name": body.name, "group_id": id})),
            );
            (StatusCode::CREATED, Json(json!({"id": id, "name": body.name, "tenant_id": body.tenant_id}))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                (StatusCode::CONFLICT, Json(json!({"error": "group with this name already exists in tenant"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
            }
        }
    }
}

async fn delete_group(Path(id): Path<String>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.execute("DELETE FROM groups_ WHERE id = ?1", [&id])
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "group not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit(None, None, audit::GROUP_DELETED, None, Some(&json!({"group_id": id})));
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn add_member(Path(id): Path<String>, Json(body): Json<AddMember>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.execute(
            "INSERT INTO user_groups (user_id, group_id) VALUES (?1, ?2)",
            rusqlite::params![body.user_id, id],
        )
    });

    match result {
        Ok(_) => {
            audit::log_audit(
                None,
                Some(&body.user_id),
                audit::GROUP_MEMBER_ADDED,
                None,
                Some(&json!({"group_id": id, "user_id": body.user_id})),
            );
            (StatusCode::CREATED, Json(json!({"ok": true}))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("PRIMARY") {
                (StatusCode::CONFLICT, Json(json!({"error": "user already in group"}))).into_response()
            } else if msg.contains("FOREIGN") {
                (StatusCode::BAD_REQUEST, Json(json!({"error": "user or group does not exist"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
            }
        }
    }
}

async fn remove_member(Path((id, user_id)): Path<(String, String)>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.execute(
            "DELETE FROM user_groups WHERE group_id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id],
        )
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "membership not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit(
                None,
                Some(&user_id),
                audit::GROUP_MEMBER_REMOVED,
                None,
                Some(&json!({"group_id": id, "user_id": user_id})),
            );
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
