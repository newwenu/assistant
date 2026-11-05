mod agent;
mod agent_mode;
mod app;
pub(crate) mod chat;
mod clock;
mod compaction;
mod config;
mod conversation;
pub(crate) mod conversation_utils;
mod mascot_asset;
mod orb;
mod os_tool_exec;
mod provider;
mod storage;
mod token_usage_record;
mod tool;
#[cfg(feature = "vault")]
pub(crate) mod vault;

pub use agent::*;
pub use agent_mode::*;
pub use app::*;
pub use chat::*;
pub use clock::*;
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
pub use vault::vault_cleanup_on_exit;

#[cfg(not(feature = "vault"))]
pub fn vault_cleanup_on_exit(_app: &tauri::AppHandle) {}

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
