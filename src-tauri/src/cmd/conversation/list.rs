use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::cmd::get_db_conn;
use crate::protocol::{ConversationDto, ConversationListChangeKindDto};
use assistant_storage::{
    conversations::{self, ConversationRow},
    providers as provider_store,
};

use super::emit_list_changed;

#[command]
pub fn create_conversation(
    app: AppHandle,
    provider_id: String,
    model_id: String,
    mode_id: Option<String>,
) -> Result<String, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;
    provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let row = ConversationRow {
        id: id.clone(),
        title: assistant_i18n::t("chat.new_conversation_title"),
        title_source: "placeholder".to_string(),
        provider_id: Some(provider_id),
        model_id,
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: now,
        updated_at: now,
        is_full: false,
        active_branch_id: None,
        mode_id,
        tool_config_json: None,
        workspace_root: String::new(),
    };
    conversations::insert_conversation(&conn, &row)?;

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::Created,
        Some(id.clone()),
        None,
    );

    Ok(id)
}

#[command]
pub fn list_conversations(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    Ok(rows.into_iter().map(Into::into).collect())
}
