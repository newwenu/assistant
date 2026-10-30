mod agent_mode;
mod app;
mod chat;
mod compaction;
mod config;
mod conversation;
mod conversation_utils;
mod mascot_asset;
mod orb;
mod os_tool_exec;
mod provider;
mod storage;
mod tool;
#[cfg(feature = "vault")]
mod vault;

pub use agent_mode::*;
pub use app::*;
pub use chat::*;
pub use compaction::*;
pub use config::*;
pub use conversation::*;

pub use mascot_asset::*;
pub use orb::*;
pub use os_tool_exec::*;
pub use provider::*;
pub use storage::*;
pub use tool::*;
#[cfg(feature = "vault")]
pub use vault::*;

pub(crate) fn get_db_conn<'a>(
    state: &'a tauri::State<'a, crate::setup::DbState>,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    state.0.lock().map_err(|e| {
        let err_str = e.to_string();
        let mut args = std::collections::HashMap::new();
        args.insert("error", err_str.as_str());
        assistant_i18n::t_with_args("error.db_lock", &args)
    })
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
