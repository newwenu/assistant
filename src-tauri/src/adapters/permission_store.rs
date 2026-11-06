use assistant_tools::{
    ApprovalKey, AuditEntry, AuditOutcome, PermissionAuditStore, PermissionGrantStore,
};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use tauri::Manager;

fn with_conn<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    f(&conn)
}

fn grant_key_json(key: &ApprovalKey) -> Result<String, String> {
    serde_json::to_string(key).map_err(|e| e.to_string())
}

fn outcome_str(outcome: AuditOutcome) -> &'static str {
    match outcome {
        AuditOutcome::Allowed => "allowed",
        AuditOutcome::GrantHit => "grantHit",
        AuditOutcome::Denied => "denied",
        AuditOutcome::Approved => "approved",
        AuditOutcome::AlwaysApproved => "alwaysApproved",
        AuditOutcome::Rejected => "rejected",
    }
}

/// 授权缓存的生产实现：`permission_grants` 以 `ApprovalKey` 的规范 JSON 为主键。
/// 读失败一律视为「未授权」（fail-closed），交由网关走审批。
pub struct DbPermissionGrantStore {
    app: tauri::AppHandle,
}

impl DbPermissionGrantStore {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl PermissionGrantStore for DbPermissionGrantStore {
    async fn is_granted(&self, key: &ApprovalKey) -> bool {
        let grant_key = match grant_key_json(key) {
            Ok(grant_key) => grant_key,
            Err(e) => {
                log::warn!("permission grant key encode failed: {e}");
                return false;
            }
        };
        match with_conn(&self.app, |conn| {
            let hits: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM permission_grants WHERE grant_key = ?1",
                    params![grant_key],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            Ok(hits > 0)
        }) {
            Ok(granted) => granted,
            Err(e) => {
                log::warn!("permission grant lookup failed: {e}");
                false
            }
        }
    }

    async fn save_grant(&self, key: &ApprovalKey) -> Result<(), String> {
        let grant_key = grant_key_json(key)?;
        let tool = key.tool.clone();
        with_conn(&self.app, |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO permission_grants (grant_key, tool, created_at)
                 VALUES (?1, ?2, ?3)",
                params![grant_key, tool, chrono::Utc::now().timestamp_millis()],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}

/// 审计的生产实现：`permission_audit` 只追加，不更新、不设 TTL。
pub struct DbPermissionAuditStore {
    app: tauri::AppHandle,
}

impl DbPermissionAuditStore {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl PermissionAuditStore for DbPermissionAuditStore {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String> {
        let outcome = outcome_str(entry.outcome);
        with_conn(&self.app, |conn| {
            conn.execute(
                "INSERT INTO permission_audit (tool, action, resource, outcome, at_millis)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    entry.tool,
                    entry.action,
                    entry.resource,
                    outcome,
                    entry.at_millis
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}
