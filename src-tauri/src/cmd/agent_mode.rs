use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::{AgentModeDto, AgentModeInputDto, ConversationToolConfigDto};
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

    // `kind` 决定已有会话的发送路径与布局，允许编辑会让同一会话静默换链路（I38d §4.8）。
    // 与既有行不一致即拒绝；换类型必须新建模式。
    let existing = if is_new {
        None
    } else {
        agent_modes::get_by_id(&conn, &mode.id)?
    };
    if let Some(row) = existing.as_ref() {
        if row.kind != mode.kind.as_str() {
            return Err(assistant_i18n::t("error.agent_mode_kind_locked"));
        }
    }

    let tool_config = match mode.tool_config {
        Some(cfg) => cfg,
        None if is_new => ConversationToolConfigDto::default(),
        None => existing
            .as_ref()
            .map(|m| AgentModeDto::from_row(m).tool_config)
            .unwrap_or_default(),
    };
    let tool_config_json = Some(serde_json::to_string(&tool_config).map_err(|e| e.to_string())?);

    // 头像三态（I43 §3.5）：缺省 / `null` 沿用现值；`""` 清除为 `NULL`；非空覆盖。
    let avatar = match mode.avatar {
        Some(raw) if raw.is_empty() => None,
        Some(raw) => Some(raw),
        None if is_new => None,
        None => existing.as_ref().and_then(|m| m.avatar.clone()),
    };

    let row = AgentModeRow {
        id: if is_new {
            Uuid::new_v4().to_string()
        } else {
            mode.id.clone()
        },
        name: mode.name,
        description: mode.description,
        kind: mode.kind.as_str().to_string(),
        prompt: mode.prompt,
        model: mode.model,
        temperature: mode.temperature,
        max_tokens: mode.max_tokens,
        reasoning_enabled: mode.reasoning_enabled,
        reasoning_effort: mode.reasoning_effort,
        presentation_rank: mode.presentation_rank,
        is_builtin: mode.is_builtin,
        created_at: mode.created_at,
        updated_at: now,
        tool_config_json,
        avatar,
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
    agent_modes::reset_builtin_mode(&conn, &id, &|key| assistant_i18n::t(key))
        .map_err(super::translate_storage_error)
}