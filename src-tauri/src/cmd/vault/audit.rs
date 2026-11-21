//! 审计日志查询

use tauri::{command, AppHandle};

use assistant_vault::audit::{clamp_query_limit, query_log};
use assistant_vault::dto::{AuditLogEntry, VaultErrorDto};
use assistant_vault::types::VaultError;

// precondition: Unlocked
#[command]
pub fn vault_query_audit_log(
    app: AppHandle,
    limit: Option<i64>,
    since: Option<i64>,
    event_types: Option<Vec<String>>,
) -> Result<Vec<AuditLogEntry>, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        let conn = guard
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::NotInitialized))?;
        let limit = clamp_query_limit(limit);
        query_log(conn, limit, since, event_types.as_deref()).map_err(VaultErrorDto::from)
    })
}
