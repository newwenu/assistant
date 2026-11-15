use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 保险库锁定原因（封闭集合，线格式为 camelCase）。
///
/// 仅用于 `vault_lock` 入参归因：后端映射为 `security_db::LockReason`，
/// 审计 detail 仍取 `LockReason::as_str()` 的 snake 串，wire 名不落库。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum LockReasonDto {
    User,
    AutoLock,
    ScreenLock,
    AppExit,
    Reset,
    Panic,
}
