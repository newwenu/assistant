mod agent;
mod agent_mode;
mod app;
mod attachment;
pub(crate) mod chat;
mod compaction;
mod config;
pub(crate) mod conversation;
mod mascot_asset;
mod mcp;
pub(crate) mod orb;
mod os_tool_exec;
mod permission;
mod provider;
mod resource;
mod screenshot;
pub mod startup;
pub mod storage;
#[cfg(feature = "timekeeper")]
pub(crate) mod timekeeper;
mod token_usage_record;
mod tool;
#[cfg(feature = "vault")]
pub(crate) mod vault;
mod workspace;

pub use agent::*;
pub use agent_mode::*;
pub use app::*;
pub use attachment::*;
pub use chat::*;
pub use compaction::*;
pub use config::*;
pub use conversation::*;

pub use mascot_asset::*;
pub use mcp::*;
pub use orb::*;
pub use os_tool_exec::*;
pub use permission::*;
pub use provider::*;
pub use resource::*;
pub use screenshot::*;
pub use startup::*;
pub use storage::*;
pub use tool::*;
pub use workspace::*;

pub(crate) type ConfApplier = fn(&tauri::AppHandle, serde_json::Value) -> Result<(), String>;

static CONF_APPLIERS: std::sync::Mutex<Vec<(&'static str, ConfApplier)>> =
    std::sync::Mutex::new(Vec::new());

static CORE_CONF_APPLIERS_SEEDED: std::sync::Once = std::sync::Once::new();

pub(crate) fn register_conf_applier(group: &'static str, applier: ConfApplier) {
    let mut registry = CONF_APPLIERS.lock().unwrap_or_else(|e| e.into_inner());
    if registry.iter().any(|(existing, _)| *existing == group) {
        log::error!("conf applier group {} 被重复注册，已忽略后一次", group);
        return;
    }
    registry.push((group, applier));
}

pub(crate) fn conf_side_effects() -> Vec<(&'static str, ConfApplier)> {
    CORE_CONF_APPLIERS_SEEDED.call_once(|| {
        register_conf_applier("app", config::apply_app_conf as ConfApplier);
        register_conf_applier("orb", config::apply_orb_conf as ConfApplier);
        register_conf_applier("mascot", config::apply_mascot_conf as ConfApplier);
    });
    let registry = CONF_APPLIERS.lock().unwrap_or_else(|e| e.into_inner());
    registry.clone()
}

/// 取连接锁的唯一实现：`error.db_lock` 文案只在这里写一份。
///
/// 命令层经 [`get_db_conn`] 转调，`cmd::storage::reclaim` 的纯函数直接传 `&Mutex<Connection>`，
/// 两处不得各写一份文案。
pub(crate) fn lock_conn<'a>(
    conn: &'a std::sync::Mutex<rusqlite::Connection>,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    conn.lock().map_err(|e| {
        let err_str = e.to_string();
        let mut args = std::collections::HashMap::new();
        args.insert("error", err_str.as_str());
        assistant_i18n::t_with_args("error.db_lock", &args)
    })
}

pub(crate) fn get_db_conn<'a>(
    state: &'a tauri::State<'a, crate::db_state::DbState>,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    lock_conn(&state.0)
}

pub(crate) fn translate_storage_error(e: assistant_storage::error::StorageError) -> String {
    use assistant_storage::error::StorageError;
    match e {
        StorageError::ConversationNotFound => assistant_i18n::t("conversation.not_found"),
        StorageError::BuiltinModeCannotDelete => {
            assistant_i18n::t("agent_mode.builtin_cannot_delete")
        }
        StorageError::NotBuiltinModeCannotReset => {
            assistant_i18n::t("agent_mode.not_builtin_cannot_reset")
        }
        StorageError::Other(s) => s,
    }
}

pub(crate) fn translate_agent_error(e: assistant_agent::error::AgentError) -> String {
    use assistant_agent::error::AgentError;
    match e {
        AgentError::AttachmentsNotAllowed => assistant_i18n::t("chat.attachments_not_allowed"),
        AgentError::TooManyImages { max } => {
            let max_s = max.to_string();
            let mut args = std::collections::HashMap::new();
            args.insert("max", max_s.as_str());
            assistant_i18n::t_with_args("chat.too_many_images", &args)
        }
        AgentError::TooManyFiles { max } => {
            let max_s = max.to_string();
            let mut args = std::collections::HashMap::new();
            args.insert("max", max_s.as_str());
            assistant_i18n::t_with_args("chat.too_many_files", &args)
        }
        AgentError::ImageTooLarge { max_mb } => {
            let max_s = max_mb.to_string();
            let mut args = std::collections::HashMap::new();
            args.insert("max_mb", max_s.as_str());
            assistant_i18n::t_with_args("chat.image_too_large", &args)
        }
        AgentError::FileTooLarge { max_mb } => {
            let max_s = max_mb.to_string();
            let mut args = std::collections::HashMap::new();
            args.insert("max_mb", max_s.as_str());
            assistant_i18n::t_with_args("chat.file_too_large", &args)
        }
        AgentError::Other(s) => s,
    }
}

/// 结构化版 `translate_agent_error`：首批命令用，未迁域继续用字符串版。
///
/// 附件类校验失败归 `Validation`（重试必然再次失败），`Other` 无法判别，
/// 显式降级为 `InternalError`。
pub(crate) fn command_error_from_agent(
    e: assistant_agent::error::AgentError,
) -> crate::protocol::CommandErrorDto {
    use crate::protocol::{CommandErrorCodeDto, CommandErrorDto};
    let code = match e {
        assistant_agent::error::AgentError::Other(_) => CommandErrorCodeDto::InternalError,
        _ => CommandErrorCodeDto::Validation,
    };
    CommandErrorDto::new(code, translate_agent_error(e))
}

/// 结构化版 `translate_storage_error`：`ConversationNotFound` 同名码，
/// 模式类为 `Validation`，`Other` 归 `StorageFailure`（本地存储故障，
/// 重试必然再次失败，前端只给「复制诊断」动作）。
pub(crate) fn command_error_from_storage(
    e: assistant_storage::error::StorageError,
) -> crate::protocol::CommandErrorDto {
    use crate::protocol::{CommandErrorCodeDto, CommandErrorDto};
    use assistant_storage::error::StorageError;
    let code = match e {
        StorageError::ConversationNotFound => CommandErrorCodeDto::ConversationNotFound,
        StorageError::BuiltinModeCannotDelete | StorageError::NotBuiltinModeCannotReset => {
            CommandErrorCodeDto::Validation
        }
        StorageError::Other(_) => CommandErrorCodeDto::StorageFailure,
    };
    CommandErrorDto::new(code, translate_storage_error(e))
}
