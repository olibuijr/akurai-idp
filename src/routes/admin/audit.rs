use axum::{Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::with_db;

#[derive(Deserialize)]
pub struct AuditQuery {
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub event: Option<String>,
    pub since: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct AuditEntry {
    pub id: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub event: String,
    pub ip: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: i64,
}

pub fn router() -> Router {
    Router::new().route("/", get(list_audit))
}

fn parse_since(s: &str) -> Option<i64> {
    // Try unix timestamp first
    if let Ok(ts) = s.parse::<i64>() {
        return Some(ts);
    }
    // Try ISO 8601 date parsing
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
    }
    None
}

async fn list_audit(Query(q): Query<AuditQuery>) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).min(1000);

    let result = with_db(|conn| {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1u32;

        if let Some(ref tid) = q.tenant_id {
            conditions.push(format!("tenant_id = ?{idx}"));
            params.push(Box::new(tid.clone()));
            idx += 1;
        }
        if let Some(ref uid) = q.user_id {
            conditions.push(format!("user_id = ?{idx}"));
            params.push(Box::new(uid.clone()));
            idx += 1;
        }
        if let Some(ref evt) = q.event {
            conditions.push(format!("event = ?{idx}"));
            params.push(Box::new(evt.clone()));
            idx += 1;
        }
        if let Some(ref since_str) = q.since
            && let Some(ts) = parse_since(since_str)
        {
            conditions.push(format!("created_at >= ?{idx}"));
            params.push(Box::new(ts));
            idx += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, tenant_id, user_id, event, ip, metadata, created_at FROM audit_log {where_clause} ORDER BY created_at DESC LIMIT ?{idx}"
        );
        params.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let metadata_raw: Option<String> = row.get(5)?;
            let metadata = metadata_raw.and_then(|s| serde_json::from_str(&s).ok());
            Ok(AuditEntry {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                user_id: row.get(2)?,
                event: row.get(3)?,
                ip: row.get(4)?,
                metadata,
                created_at: row.get(6)?,
            })
        })?;
        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok::<_, rusqlite::Error>(entries)
    });

    match result {
        Ok(entries) => Json(json!(entries)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
