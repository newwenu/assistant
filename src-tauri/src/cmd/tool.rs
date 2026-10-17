use tauri::{command, AppHandle, Manager};

use assistant_storage::{tool_favorites, tool_history};

#[command]
pub fn record_tool_usage(app: AppHandle, tool_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    tool_history::record_usage(&conn, &tool_id)?;
    let _ = tool_history::cleanup_old_history(&conn);
    Ok(())
}

#[command]
pub fn get_recent_tools(app: AppHandle, limit: Option<i32>) -> Result<Vec<String>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let limit = limit.unwrap_or(10);
    tool_history::get_recent_tools(&conn, limit)
}

#[command]
pub fn get_favorite_tools(app: AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    tool_favorites::get_favorites(&conn)
}

#[command]
pub fn toggle_tool_favorite(app: AppHandle, tool_id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    tool_favorites::toggle_favorite(&conn, &tool_id)
}

#[command]
pub fn set_favorite_tools_order(app: AppHandle, tool_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    tool_favorites::set_favorites_order(&conn, &tool_ids)
}
