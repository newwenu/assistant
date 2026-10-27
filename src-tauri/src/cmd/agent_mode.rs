use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::{AgentModeDto, AgentModeInputDto};
use assistant_storage::agent_modes::{self, AgentModeRow};

fn row_to_dto(row: &AgentModeRow) -> AgentModeDto {
    AgentModeDto::from_row(row)
}

#[command]
pub fn list_agent_modes(app: AppHandle) -> Result<Vec<AgentModeDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let modes = agent_modes::get_modes(&conn)?;
    Ok(modes.iter().map(row_to_dto).collect())
}

#[command]
pub fn save_agent_mode(app: AppHandle, mode: AgentModeInputDto) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let is_new = mode.id.is_empty();
    let now = Utc::now().timestamp_millis();

    let row = AgentModeRow {
        id: if is_new {
            Uuid::new_v4().to_string()
        } else {
            mode.id.clone()
        },
        name: mode.name,
        description: mode.description,
        kind: mode.kind,
        prompt: mode.prompt,
        model: mode.model,
        temperature: mode.temperature,
        max_tokens: mode.max_tokens,
        reasoning_enabled: mode.reasoning_enabled,
        reasoning_effort: mode.reasoning_effort,
        tools_json: serde_json::to_string(&mode.tools).unwrap_or_else(|_| "[]".to_string()),
        presentation_rank: mode.presentation_rank,
        is_builtin: mode.is_builtin,
        created_at: mode.created_at,
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
    agent_modes::delete_mode(&conn, &id).map_err(super::translate_storage_error)
}

#[command]
pub fn reset_builtin_mode(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    agent_modes::reset_builtin_mode(&conn, &id).map_err(super::translate_storage_error)
}
