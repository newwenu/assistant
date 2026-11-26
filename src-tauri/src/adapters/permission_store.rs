use assistant_tools::{
    ApprovalKey, AuditEntry, AuditOutcome, GrantScope, PermissionAuditStore, PermissionGrantStore,
};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use tauri::Manager;

const GLOBAL_PREFIX: &str = "g|";
const SESSION_PREFIX: &str = "s:";

fn with_conn<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    f(&conn)
}

fn grant_key_json(key: &ApprovalKey) -> Result<String, String> {
    serde_json::to_string(key).map_err(|e| e.to_string())
}

/// 按作用域编码存储主键：全局 `g|<json>`，会话级 `s:<conversationId>|<json>`。
pub(crate) fn encode_grant_key(
    json: &str,
    scope: GrantScope,
    conversation_id: Option<&str>,
) -> Result<String, String> {
    match scope {
        GrantScope::Global => Ok(format!("{GLOBAL_PREFIX}{json}")),
        GrantScope::Session => {
            let conversation_id = conversation_id
                .ok_or_else(|| "session grant requires conversation id".to_string())?;
            Ok(format!("{SESSION_PREFIX}{conversation_id}|{json}"))
        }
    }
}

/// 解析存储主键的「作用域前缀 + 键 JSON」。
///
/// 历史无前缀行按 `global` 兼容读取，不迁移、不改写。
pub(crate) fn decode_grant_key(raw: &str) -> (GrantScope, Option<&str>, &str) {
    if let Some(rest) = raw.strip_prefix(GLOBAL_PREFIX) {
        (GrantScope::Global, None, rest)
    } else if let Some(rest) = raw.strip_prefix(SESSION_PREFIX) {
        match rest.split_once('|') {
            Some((conversation_id, json)) => (GrantScope::Session, Some(conversation_id), json),
            None => (GrantScope::Global, None, raw),
        }
    } else {
        (GrantScope::Global, None, raw)
    }
}

pub(crate) fn outcome_str(outcome: AuditOutcome) -> &'static str {
    match outcome {
        AuditOutcome::Allowed => "allowed",
        AuditOutcome::GrantHit => "grantHit",
        AuditOutcome::Denied => "denied",
        AuditOutcome::Approved => "approved",
        AuditOutcome::SessionApproved => "sessionApproved",
        AuditOutcome::AlwaysApproved => "alwaysApproved",
        AuditOutcome::Rejected => "rejected",
        AuditOutcome::TimedOut => "timedOut",
        AuditOutcome::ValidateRejected => "validateRejected",
        AuditOutcome::IntentError => "intentError",
        AuditOutcome::Bypassed => "bypassed",
    }
}

/// 授权缓存的生产实现：`permission_grants` 主键带作用域前缀（见 [`encode_grant_key`]）。
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
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool {
        let json = match grant_key_json(key) {
            Ok(json) => json,
            Err(e) => {
                log::warn!("permission grant key encode failed: {e}");
                return false;
            }
        };
        let global = encode_grant_key(&json, GrantScope::Global, None)
            .unwrap_or_else(|_| format!("{GLOBAL_PREFIX}{json}"));
        let session = conversation_id
            .and_then(|id| encode_grant_key(&json, GrantScope::Session, Some(id)).ok())
            .unwrap_or_default();
        match with_conn(&self.app, |conn| {
            let hits: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM permission_grants WHERE grant_key = ?1 OR grant_key = ?2",
                    params![global, session],
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

    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String> {
        let json = grant_key_json(key)?;
        let grant_key = encode_grant_key(&json, scope, conversation_id)?;
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

/// 审计的生产实现：`permission_audit` 只追加，不更新；过期行由启动清理按
/// `audit_retention_days` 删除（`0` 表示不清理）。
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
        let source = entry.source.map(|value| value.as_str());
        with_conn(&self.app, |conn| {
            conn.execute(
                "INSERT INTO permission_audit
                   (tool, action, resource, outcome, at_millis, conversation_id, turn_id, iteration, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    entry.tool,
                    entry.action,
                    entry.resource,
                    outcome,
                    entry.at_millis,
                    entry.conversation_id,
                    entry.turn_id,
                    entry.iteration.map(i64::from),
                    source
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}
