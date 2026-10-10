use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use assistant_storage::agent_modes::{self, AgentModeRow};

#[command]
pub fn list_agent_modes(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let modes = agent_modes::get_modes(&conn)?;
    serde_json::to_value(&modes).map_err(|e| e.to_string())
}

#[command]
pub fn save_agent_mode(app: AppHandle, mode: serde_json::Value) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let id = mode.get("id").and_then(|v| v.as_str()).unwrap_or("");

    let is_new = id.is_empty();
    let now = Utc::now().timestamp_millis();

    let row = AgentModeRow {
        id: if is_new {
            Uuid::new_v4().to_string()
        } else {
            id.to_string()
        },
        name: mode
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        description: mode
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        kind: mode
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_string(),
        prompt: mode
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: mode
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        temperature: mode
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32),
        max_tokens: mode
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        reasoning_enabled: mode.get("reasoning_enabled").and_then(|v| v.as_bool()),
        reasoning_effort: mode
            .get("reasoning_effort")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        tools_json: mode
            .get("tools")
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()))
            .unwrap_or_else(|| "[]".to_string()),
        presentation_rank: mode
            .get("presentation_rank")
            .and_then(|v| v.as_u64())
            .unwrap_or(99) as u32,
        is_builtin: mode
            .get("is_builtin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        created_at: mode
            .get("created_at")
            .and_then(|v| v.as_i64())
            .unwrap_or(now),
        updated_at: now,
    };

    if is_new {
        agent_modes::insert_mode(&conn, &row)?;
    } else {
        agent_modes::update_mode(&conn, &row)?;
    }
    Ok(())
}

#[command]
pub fn delete_agent_mode(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    agent_modes::delete_mode(&conn, &id)
}

#[command]
pub fn reset_builtin_mode(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    agent_modes::reset_builtin_mode(&conn, &id)
}
