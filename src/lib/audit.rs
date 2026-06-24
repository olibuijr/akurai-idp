use rusqlite::Connection;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Event type constants
// ---------------------------------------------------------------------------
pub const USER_LOGIN: &str = "USER_LOGIN";
pub const USER_LOGIN_FAILED: &str = "USER_LOGIN_FAILED";
pub const USER_CREATED: &str = "USER_CREATED";
pub const USER_DELETED: &str = "USER_DELETED";
pub const USER_LOCKED: &str = "USER_LOCKED";
pub const USER_PASSWORD_CHANGED: &str = "USER_PASSWORD_CHANGED";
pub const USER_MFA_ENABLED: &str = "USER_MFA_ENABLED";
pub const USER_MFA_DISABLED: &str = "USER_MFA_DISABLED";
pub const TOKEN_ISSUED: &str = "TOKEN_ISSUED";
pub const TOKEN_REVOKED: &str = "TOKEN_REVOKED";
pub const CLIENT_CREATED: &str = "CLIENT_CREATED";
pub const CLIENT_DELETED: &str = "CLIENT_DELETED";
pub const CLIENT_SECRET_ROTATED: &str = "CLIENT_SECRET_ROTATED";
pub const SESSION_CREATED: &str = "SESSION_CREATED";
pub const SESSION_REVOKED: &str = "SESSION_REVOKED";
pub const USER_UPDATED: &str = "USER_UPDATED";
pub const GROUP_CREATED: &str = "GROUP_CREATED";
pub const GROUP_DELETED: &str = "GROUP_DELETED";
pub const GROUP_MEMBER_ADDED: &str = "GROUP_MEMBER_ADDED";
pub const GROUP_MEMBER_REMOVED: &str = "GROUP_MEMBER_REMOVED";
pub const TENANT_CREATED: &str = "TENANT_CREATED";
pub const TENANT_UPDATED: &str = "TENANT_UPDATED";
pub const TENANT_DELETED: &str = "TENANT_DELETED";
pub const AUTH_CODE_ISSUED: &str = "AUTH_CODE_ISSUED";
pub const CLIENT_CREDENTIALS_ISSUED: &str = "CLIENT_CREDENTIALS_ISSUED";

// ---------------------------------------------------------------------------
// log_audit_event
// ---------------------------------------------------------------------------

/// Insert an audit log entry. Fire-and-forget: errors are logged to stderr
/// but never propagated, so audit failures cannot break the calling request.
pub fn log_audit_event(
    conn: &Connection,
    tenant_id: Option<&str>,
    user_id: Option<&str>,
    event: &str,
    ip: Option<&str>,
    metadata: Option<&Value>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let metadata_json = metadata.map(|v| v.to_string());

    let result = conn.execute(
        "INSERT INTO audit_log (id, tenant_id, user_id, event, ip, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, tenant_id, user_id, event, ip, metadata_json, now],
    );

    if let Err(e) = result {
        eprintln!("[audit] failed to write audit event {event}: {e}");
    }
}

/// Convenience wrapper: acquires a DB connection internally.
/// Use this from code that runs outside a `with_db` closure.
pub fn log_audit(
    tenant_id: Option<&str>,
    user_id: Option<&str>,
    event: &str,
    ip: Option<&str>,
    metadata: Option<&Value>,
) {
    crate::db::with_db(|conn| {
        log_audit_event(conn, tenant_id, user_id, event, ip, metadata);
    });
}
