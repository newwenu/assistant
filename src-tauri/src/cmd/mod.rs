mod config;
mod orb;
mod provider;
mod conversation;
mod chat;
mod tool;
mod agent_mode;
mod app;

pub use config::*;
pub use orb::*;
pub use provider::*;
pub use conversation::*;
pub use chat::*;
pub use tool::*;
pub use agent_mode::*;
pub use app::*;

pub(crate) fn get_db_conn<'a>(
    state: &'a tauri::State<'a, crate::setup::DbState>,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    state.0.lock().map_err(|e| format!("DB lock: {}", e))
}