use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::with_db;
use crate::lib::audit;
use crate::lib::crypto::generate_secure_token;

#[derive(Serialize)]
pub struct TenantRow {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub created_at: i64,
}

#[derive(Deserialize)]
pub struct CreateTenant {
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateTenant {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub domain: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_tenants).post(create_tenant))
        .route("/{id}", patch(update_tenant).delete(delete_tenant))
}

async fn list_tenants() -> impl IntoResponse {
    let result = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT id, name, slug, domain, created_at FROM tenants")?;
        let rows = stmt.query_map([], |row| {
            Ok(TenantRow {
                id: row.get(0)?,
                name: row.get(1)?,
                slug: row.get(2)?,
                domain: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut tenants = Vec::new();
        for r in rows {
            tenants.push(r?);
        }
        Ok::<_, rusqlite::Error>(tenants)
    });

    match result {
        Ok(tenants) => Json(json!(tenants)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn create_tenant(Json(body): Json<CreateTenant>) -> impl IntoResponse {
    let id = generate_secure_token(16);
    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        conn.execute(
            "INSERT INTO tenants (id, name, slug, domain, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, body.name, body.slug, body.domain, now],
        )
    });

    match result {
        Ok(_) => {
            audit::log_audit_event(
                Some(&id),
                None,
                audit::TENANT_CREATED,
                None,
                Some(&json!({"name": body.name, "slug": body.slug}).to_string()),
            );
            (StatusCode::CREATED, Json(json!({"id": id, "name": body.name, "slug": body.slug}))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                (StatusCode::CONFLICT, Json(json!({"error": "tenant with this slug already exists"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
            }
        }
    }
}

async fn update_tenant(Path(id): Path<String>, Json(body): Json<UpdateTenant>) -> impl IntoResponse {
    let result = with_db(|conn| {
        let mut sets = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1u32;

        if let Some(ref name) = body.name {
            sets.push(format!("name = ?{idx}"));
            params.push(Box::new(name.clone()));
            idx += 1;
        }
        if let Some(ref slug) = body.slug {
            sets.push(format!("slug = ?{idx}"));
            params.push(Box::new(slug.clone()));
            idx += 1;
        }
        if let Some(ref domain) = body.domain {
            sets.push(format!("domain = ?{idx}"));
            params.push(Box::new(domain.clone()));
            idx += 1;
        }

        if sets.is_empty() {
            return Ok(0);
        }

        let sql = format!("UPDATE tenants SET {} WHERE id = ?{idx}", sets.join(", "));
        params.push(Box::new(id.clone()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "tenant not found or no changes"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(Some(&id), None, audit::TENANT_UPDATED, None, None);
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                (StatusCode::CONFLICT, Json(json!({"error": "slug already taken"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
            }
        }
    }
}

async fn delete_tenant(Path(id): Path<String>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.execute("DELETE FROM tenants WHERE id = ?1", [&id])
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "tenant not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(Some(&id), None, audit::TENANT_DELETED, None, None);
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
