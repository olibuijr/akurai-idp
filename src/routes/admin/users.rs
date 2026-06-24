use axum::{
    extract::{Path, Query},
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
use crate::lib::password::hash_password;

#[derive(Deserialize)]
pub struct ListQuery {
    pub tenant_id: Option<String>,
}

#[derive(Serialize)]
pub struct UserRow {
    pub id: String,
    pub tenant_id: String,
    pub email: String,
    pub email_verified: bool,
    pub mfa_enabled: bool,
    pub force_password_change: bool,
    pub locked_until: Option<i64>,
    pub failed_attempts: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub email: String,
    pub password: String,
    pub tenant_id: String,
}

#[derive(Deserialize)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub mfa_enabled: Option<bool>,
    pub force_password_change: Option<bool>,
}

#[derive(Deserialize)]
pub struct ResetPassword {
    pub password: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/{id}", get(get_user).patch(update_user).delete(delete_user))
        .route("/{id}/reset-password", post(reset_password))
        .route("/{id}/lock", post(lock_user))
        .route("/{id}/unlock", post(unlock_user))
}

async fn list_users(Query(q): Query<ListQuery>) -> impl IntoResponse {
    let result = with_db(|conn| {
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &q.tenant_id {
            Some(tid) => (
                "SELECT id, tenant_id, email, email_verified, mfa_enabled, force_password_change, locked_until, failed_attempts, created_at, updated_at FROM users WHERE tenant_id = ?1".to_string(),
                vec![Box::new(tid.clone()) as Box<dyn rusqlite::types::ToSql>],
            ),
            None => (
                "SELECT id, tenant_id, email, email_verified, mfa_enabled, force_password_change, locked_until, failed_attempts, created_at, updated_at FROM users".to_string(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(UserRow {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                email: row.get(2)?,
                email_verified: row.get::<_, i64>(3)? != 0,
                mfa_enabled: row.get::<_, i64>(4)? != 0,
                force_password_change: row.get::<_, i64>(5)? != 0,
                locked_until: row.get(6)?,
                failed_attempts: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        let mut users = Vec::new();
        for r in rows {
            users.push(r?);
        }
        Ok::<_, rusqlite::Error>(users)
    });

    match result {
        Ok(users) => Json(json!(users)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn get_user(Path(id): Path<String>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.query_row(
            "SELECT id, tenant_id, email, email_verified, mfa_enabled, force_password_change, locked_until, failed_attempts, created_at, updated_at FROM users WHERE id = ?1",
            [&id],
            |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    email: row.get(2)?,
                    email_verified: row.get::<_, i64>(3)? != 0,
                    mfa_enabled: row.get::<_, i64>(4)? != 0,
                    force_password_change: row.get::<_, i64>(5)? != 0,
                    locked_until: row.get(6)?,
                    failed_attempts: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
    });

    match result {
        Ok(user) => Json(json!(user)).into_response(),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn create_user(Json(body): Json<CreateUser>) -> impl IntoResponse {
    let pw_hash = match hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    let id = generate_secure_token(16);
    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        conn.execute(
            "INSERT INTO users (id, tenant_id, email, password_hash, email_verified, mfa_enabled, force_password_change, failed_attempts, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, ?5, ?5)",
            rusqlite::params![id, body.tenant_id, body.email, pw_hash, now],
        )
    });

    match result {
        Ok(_) => {
            audit::log_audit_event(
                Some(&body.tenant_id),
                Some(&id),
                audit::USER_CREATED,
                None,
                Some(&json!({"email": body.email}).to_string()),
            );
            (StatusCode::CREATED, Json(json!({"id": id, "email": body.email, "tenant_id": body.tenant_id}))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                (StatusCode::CONFLICT, Json(json!({"error": "user with this email already exists in tenant"}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
            }
        }
    }
}

async fn update_user(Path(id): Path<String>, Json(body): Json<UpdateUser>) -> impl IntoResponse {
    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        let mut sets = vec!["updated_at = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
        let mut idx = 2u32;

        if let Some(ref email) = body.email {
            sets.push(format!("email = ?{idx}"));
            params.push(Box::new(email.clone()));
            idx += 1;
        }
        if let Some(verified) = body.email_verified {
            sets.push(format!("email_verified = ?{idx}"));
            params.push(Box::new(verified as i64));
            idx += 1;
        }
        if let Some(mfa) = body.mfa_enabled {
            sets.push(format!("mfa_enabled = ?{idx}"));
            params.push(Box::new(mfa as i64));
            idx += 1;
        }
        if let Some(force) = body.force_password_change {
            sets.push(format!("force_password_change = ?{idx}"));
            params.push(Box::new(force as i64));
            idx += 1;
        }

        if sets.len() == 1 {
            return Ok(0); // nothing to update besides updated_at
        }

        let sql = format!("UPDATE users SET {} WHERE id = ?{idx}", sets.join(", "));
        params.push(Box::new(id.clone()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "user not found or no changes"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(None, Some(&id), audit::USER_UPDATED, None, None);
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn delete_user(Path(id): Path<String>) -> impl IntoResponse {
    let result = with_db(|conn| {
        conn.execute("DELETE FROM users WHERE id = ?1", [&id])
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(None, Some(&id), audit::USER_DELETED, None, None);
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn reset_password(Path(id): Path<String>, Json(body): Json<ResetPassword>) -> impl IntoResponse {
    let pw_hash = match hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    };

    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        conn.execute(
            "UPDATE users SET password_hash = ?1, failed_attempts = 0, locked_until = NULL, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![pw_hash, now, id],
        )
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(None, Some(&id), audit::USER_PASSWORD_CHANGED, None, None);
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn lock_user(Path(id): Path<String>) -> impl IntoResponse {
    // 100 years from now
    let locked_until = chrono::Utc::now().timestamp() + 100 * 365 * 24 * 3600;
    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        conn.execute(
            "UPDATE users SET locked_until = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![locked_until, now, id],
        )
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(None, Some(&id), audit::USER_LOCKED, None, None);
            Json(json!({"ok": true, "locked_until": locked_until})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn unlock_user(Path(id): Path<String>) -> impl IntoResponse {
    let now = chrono::Utc::now().timestamp();

    let result = with_db(|conn| {
        conn.execute(
            "UPDATE users SET locked_until = NULL, failed_attempts = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
    });

    match result {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "user not found"}))).into_response(),
        Ok(_) => {
            audit::log_audit_event(None, Some(&id), audit::USER_UPDATED, None, Some("{\"action\":\"unlock\"}"));
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
