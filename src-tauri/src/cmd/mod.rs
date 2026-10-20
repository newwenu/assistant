mod agent_mode;
mod app;
mod chat;
mod chat_stream;
mod chat_utils;
mod compaction;
mod config;
mod conversation;
mod conversation_utils;
mod mascot_asset;
mod orb;
mod os_tool_exec;
mod provider;
mod tool;

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
pub use tool::*;

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
