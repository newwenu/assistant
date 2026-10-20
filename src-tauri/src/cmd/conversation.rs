use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ChatMessagesChangedPayloadDto, ConversationDetailDto, ConversationDto,
    ConversationListChangedPayloadDto, MessageDto, TokenUsageSummaryDto,
};
use assistant_storage::{
    agent_modes,
    conversations::{self, ConversationRow},
    providers::{self as provider_store},
};

fn emit_list_changed(
    app: &AppHandle,
    kind: &str,
    id: Option<String>,
    affected: Option<Vec<String>>,
) {
    emit_broadcast(
        app,
        event_name::CONVERSATION_LIST_CHANGED,
        ConversationListChangedPayloadDto {
            change_kind: kind.to_string(),
            conversation_id: id,
            affected_ids: affected,
        },
    );
}

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
    };
    conversations::insert_conversation(&conn, &row)?;

    if let Some(mid) = mode_id {
        let _ = conn.execute(
            "UPDATE conversations SET mode_id = ?1 WHERE id = ?2",
            rusqlite::params![mid, id],
        );
    }

    emit_list_changed(&app, "created", Some(id.clone()), None);

    Ok(id)
}

#[command]
pub fn list_conversations(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[command]
pub fn get_conversation(
    app: AppHandle,
    id: String,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let detail = conversations::get_conversation_detail(&conn, &id)?;
    match detail {
        Some(d) => {
            let total_count = d.messages.len() as i64;
            let msg_ids: Vec<String> = d.messages.iter().map(|m| m.id.clone()).collect();
            let sibling_counts = conversations::get_sibling_counts_batch(&conn, &id, &msg_ids)?;
            let messages: Vec<MessageDto> = d
                .messages
                .into_iter()
                .map(|m| {
                    let mut dto = MessageDto::from(m);
                    dto.sibling_count = sibling_counts.get(&dto.id).copied();
                    dto
                })
                .collect();
            Ok(Some(ConversationDetailDto {
                conversation: d.conversation.into(),
                messages,
                total_count,
            }))
        }
        None => Ok(None),
    }
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)?;

    emit_list_changed(&app, "deleted", Some(id), None);

    Ok(())
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
    conversations::rename_conversation(&conn, &id, &title, &source)?;

    emit_list_changed(&app, "renamed", Some(id), None);

    Ok(())
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
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    conv.model_id = model_id;
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)?;

    emit_list_changed(&app, "model_updated", Some(id), None);

    Ok(())
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
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    conv.provider_id = Some(provider_id);
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)?;

    emit_list_changed(&app, "provider_updated", Some(id), None);

    Ok(())
}

#[command]
pub fn get_conversation_paginated(
    app: AppHandle,
    id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    match conversations::get_conversation_by_id(&conn, &id)? {
        Some(conv) => {
            let page_limit = limit.unwrap_or(50);
            let (messages, total_count) = if let Some(ref leaf_id) = conv.active_branch_id {
                let branch = conversations::trace_active_branch(&conn, &id, leaf_id)?;
                let count = conversations::get_message_count(&conn, &id)?;
                (branch, count)
            } else {
                let msgs = conversations::get_messages_paginated(
                    &conn,
                    &id,
                    page_limit,
                    before_timestamp,
                )?;
                let count = conversations::get_message_count(&conn, &id)?;
                (msgs, count)
            };
            let msg_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
            let sibling_counts = conversations::get_sibling_counts_batch(&conn, &id, &msg_ids)?;
            let message_dtos: Vec<MessageDto> = messages
                .into_iter()
                .map(|m| {
                    let mut dto = MessageDto::from(m);
                    dto.sibling_count = sibling_counts.get(&dto.id).copied();
                    dto
                })
                .collect();
            Ok(Some(ConversationDetailDto {
                conversation: conv.into(),
                messages: message_dtos,
                total_count,
            }))
        }
        None => Ok(None),
    }
}

#[command]
pub fn get_conversation_all_messages(
    app: AppHandle,
    id: String,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    match conversations::get_conversation_by_id(&conn, &id)? {
        Some(conv) => {
            let messages = conversations::get_messages(&conn, &id)?;
            let total_count = messages.len() as i64;
            let msg_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
            let sibling_counts = conversations::get_sibling_counts_batch(&conn, &id, &msg_ids)?;
            let message_dtos: Vec<MessageDto> = messages
                .into_iter()
                .map(|m| {
                    let mut dto = MessageDto::from(m);
                    dto.sibling_count = sibling_counts.get(&dto.id).copied();
                    dto
                })
                .collect();
            Ok(Some(ConversationDetailDto {
                conversation: conv.into(),
                messages: message_dtos,
                total_count,
            }))
        }
        None => Ok(None),
    }
}

#[command]
pub fn get_message_subtree_size(app: AppHandle, message_id: String) -> Result<i64, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::get_subtree_size(&conn, &message_id)
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let conv_id = conversations::get_conversation_id_by_message(&conn, &message_id)?;
    conversations::delete_subtree(&conn, &message_id)?;

    if let Some(ref cid) = conv_id {
        if let Some(detail) = conversations::get_conversation_detail(&conn, cid)? {
            emit_broadcast(
                &app,
                event_name::CHAT_MESSAGES_CHANGED,
                ChatMessagesChangedPayloadDto {
                    conversation_id: cid.clone(),
                    change_kind: "message_deleted".to_string(),
                    messages: super::conversation_utils::messages_with_sibling_counts(
                        &conn,
                        cid,
                        &detail.messages,
                    )?,
                    active_branch_id: detail.conversation.active_branch_id.clone(),
                    _seq: None,
                },
            );
        }
    }
    Ok(())
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    Ok(rows.into_iter().map(Into::into).collect())
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
pub fn export_conversation_to_file(
    app: AppHandle,
    id: String,
    format: String,
    path: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let content = match format.as_str() {
        "json" => conversations::export_conversation_json(&conn, &id)?,
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id)?,
        _ => conversations::export_conversation_markdown(&conn, &id)?,
    };
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let result = conversations::toggle_pin(&conn, &id)?;

    emit_list_changed(&app, "pin_toggled", Some(id), None);

    Ok(result)
}

#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let new_id = conversations::duplicate_conversation(&conn, &id)?;

    emit_list_changed(&app, "duplicated", Some(new_id.clone()), None);

    Ok(new_id)
}

#[command]
pub fn batch_delete_conversations(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let affected = ids.clone();
    conversations::batch_delete(&conn, &ids)?;

    emit_list_changed(&app, "batch_deleted", None, Some(affected));

    Ok(())
}

#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::purge_all(&conn)?;

    emit_list_changed(&app, "purged", None, None);

    Ok(())
}

#[command]
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<TokenUsageSummaryDto, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let ctx = if let Some(c) = context_window_tokens {
        c
    } else {
        super::conversation_utils::resolve_context_window_for_conversation(&conn, &conversation_id)
    };
    let summary = agent_modes::get_conversation_token_usage(&conn, &conversation_id, ctx)?;
    Ok(summary.into())
}

#[command]
pub fn create_branch(
    app: AppHandle,
    conversation_id: String,
    branch_point_message_id: String,
    new_role: String,
    new_content: String,
    _seq: Option<u32>,
) -> Result<crate::protocol::MessageDto, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let new_msg = conversations::create_branch(
        &conn,
        &conversation_id,
        &branch_point_message_id,
        &new_role,
        &new_content,
    )?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        crate::protocol::ChatMessagesChangedPayloadDto {
            conversation_id: conversation_id.clone(),
            change_kind: "branch_created".to_string(),
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            _seq,
        },
    );

    Ok(crate::protocol::MessageDto::from(new_msg))
}

#[command]
pub fn switch_branch(
    app: AppHandle,
    conversation_id: String,
    target_message_id: String,
    _seq: Option<u32>,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    conversations::switch_branch(&conn, &conversation_id, &target_message_id)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        crate::protocol::ChatMessagesChangedPayloadDto {
            conversation_id: conversation_id.clone(),
            change_kind: "branch_switched".to_string(),
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            _seq,
        },
    );

    Ok(())
}

#[command]
pub fn get_message_siblings(
    app: AppHandle,
    message_id: String,
) -> Result<Vec<crate::protocol::SiblingInfoDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let siblings = conversations::get_message_siblings(&conn, &message_id)?;
    Ok(siblings
        .into_iter()
        .map(crate::protocol::SiblingInfoDto::from)
        .collect())
}

#[command]
pub fn increment_generation(app: AppHandle, id: String) -> Result<i64, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::increment_generation(&conn, &id)
}

#[command]
pub fn fork_to_new_conversation(
    app: AppHandle,
    source_conversation_id: String,
    branch_leaf_message_id: String,
) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let new_conv_id = conversations::fork_to_new_conversation(
        &conn,
        &source_conversation_id,
        &branch_leaf_message_id,
    )?;

    let detail = conversations::get_conversation_detail(&conn, &source_conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        crate::protocol::ChatMessagesChangedPayloadDto {
            conversation_id: source_conversation_id.clone(),
            change_kind: "branch_forked".to_string(),
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &source_conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            _seq: None,
        },
    );

    emit_list_changed(&app, "created", Some(new_conv_id.clone()), None);

    Ok(new_conv_id)
}
