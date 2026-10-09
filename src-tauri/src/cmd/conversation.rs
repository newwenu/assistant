use chrono::Utc;
use serde_json::json;
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use assistant_storage::{
    agent_modes,
    conversations::{self, ConversationRow},
    providers::{self as provider_store},
};

#[command]
pub fn create_conversation(
    app: AppHandle,
    provider_id: String,
    model_id: String,
    mode_id: Option<String>,
) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let row = ConversationRow {
        id: id.clone(),
        title: "新对话".to_string(),
        title_source: "placeholder".to_string(),
        provider_id: Some(provider_id),
        model_id,
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: now,
        updated_at: now,
    };
    conversations::insert_conversation(&conn, &row)?;

    if let Some(mid) = mode_id {
        let _ = conn.execute(
            "UPDATE conversations SET mode_id = ?1 WHERE id = ?2",
            rusqlite::params![mid, id],
        );
    }

    Ok(id)
}

#[command]
pub fn list_conversations(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    serde_json::to_value(&rows).map_err(|e| e.to_string())
}

#[command]
pub fn get_conversation(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let detail = conversations::get_conversation_detail(&conn, &id)?;
    serde_json::to_value(&detail).map_err(|e| e.to_string())
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)
}

#[command]
pub fn rename_conversation(
    app: AppHandle,
    id: String,
    title: String,
    title_source: Option<String>,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let source = title_source.unwrap_or_else(|| "user".to_string());
    conversations::rename_conversation(&conn, &id, &title, &source)
}

#[command]
pub fn update_conversation_model(
    app: AppHandle,
    id: String,
    model_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut conv = conversations::get_conversation_by_id(&conn, &id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    conv.model_id = model_id;
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)
}

#[command]
pub fn update_conversation_provider(
    app: AppHandle,
    id: String,
    provider_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut conv = conversations::get_conversation_by_id(&conn, &id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    conv.provider_id = Some(provider_id);
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)
}

#[command]
pub fn get_conversation_paginated(
    app: AppHandle,
    id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    match conversations::get_conversation_by_id(&conn, &id)? {
        Some(conv) => {
            let page_limit = limit.unwrap_or(50);
            let messages =
                conversations::get_messages_paginated(&conn, &id, page_limit, before_timestamp)?;
            let total_count = conversations::get_message_count(&conn, &id)?;
            Ok(json!({
                "conversation": conv,
                "messages": messages,
                "total_count": total_count,
            }))
        }
        None => Ok(json!(null)),
    }
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_message(&conn, &message_id)
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    serde_json::to_value(&rows).map_err(|e| e.to_string())
}

#[command]
pub fn export_conversation(app: AppHandle, id: String, format: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    match format.as_str() {
        "json" => conversations::export_conversation_json(&conn, &id),
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id),
        _ => conversations::export_conversation_markdown(&conn, &id),
    }
}

#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::toggle_pin(&conn, &id)
}

#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::duplicate_conversation(&conn, &id)
}

#[command]
pub fn batch_delete_conversations(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::batch_delete(&conn, &ids)
}

#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::purge_all(&conn)
}

#[command]
pub fn edit_message(app: AppHandle, message_id: String, new_content: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::edit_message(&conn, &message_id, &new_content)
}

#[command]
pub fn regenerate_message(
    app: AppHandle,
    conversation_id: String,
    message_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_message_and_after(&conn, &message_id)?;
    let _ = app.emit(
        "chat-regenerate",
        json!({
            "conversation_id": conversation_id,
            "deleted_message_id": message_id,
        }),
    );
    Ok(())
}

#[command]
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let ctx = context_window_tokens.unwrap_or(128_000);
    let summary = agent_modes::get_conversation_token_usage(&conn, &conversation_id, ctx)?;
    serde_json::to_value(&summary).map_err(|e| e.to_string())
}