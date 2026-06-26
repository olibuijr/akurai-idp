use axum::{Json, Router, routing::get};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};

use crate::db::with_db;
use crate::middleware::auth::AuthUser;

pub fn router() -> Router {
    Router::new().route("/userinfo", get(userinfo_endpoint))
}

async fn userinfo_endpoint(request: Request) -> impl IntoResponse {
    let user = match request.extensions().get::<AuthUser>() {
        Some(u) => u.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "unauthorized", "error_description": "Authentication required"})),
            )
                .into_response();
        }
    };

    let groups: Vec<String> = with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT g.name FROM groups g
                 JOIN user_groups ug ON ug.group_id = g.id
                 WHERE ug.user_id = ?1",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![user.id], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    });

    let claims: Value = json!({
        "sub": user.id,
        "email": user.email,
        "email_verified": user.email_verified,
        "tenant_id": user.tenant_id,
        "groups": groups,
    });

    (StatusCode::OK, Json(claims)).into_response()
}
