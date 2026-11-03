use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ChatMessagesChangedPayloadDto, ConversationDetailDto, ConversationDto,
    ConversationListChangeKindDto, ConversationListChangedPayloadDto, ConversationPatchDto,
    ConversationQueryOptsDto, MessageChangeKindDto, MessageDto, TokenUsageEstimatedSourceDto,
    TokenUsageSummaryDto,
};
use assistant_agent::chat_request;
use assistant_storage::{
    attachment_store::AttachmentStore,
    conversations::{self, ConversationRow, MessageRow},
    providers::{self as provider_store},
};

fn emit_list_changed(
    app: &AppHandle,
    kind: ConversationListChangeKindDto,
    id: Option<String>,
    affected: Option<Vec<String>>,
) {
    emit_broadcast(
        app,
        event_name::CONVERSATION_LIST_CHANGED,
        ConversationListChangedPayloadDto {
            change_kind: kind,
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
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[command]
pub fn get_conversation(
    app: AppHandle,
    id: String,
    opts: Option<ConversationQueryOptsDto>,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let mode = opts.as_ref().map(|o| o.mode.as_str()).unwrap_or("detail");

    match mode {
        "all" => get_conversation_all_impl(&conn, &id),
        "paginated" => {
            let o = opts.as_ref();
            let limit = o.map(|o| o.limit).unwrap_or(50);
            let before_ts = o.and_then(|o| o.before_timestamp);
            get_conversation_paginated_impl(&conn, &id, limit, before_ts)
        }
        _ => get_conversation_detail_impl(&conn, &id),
    }
}

fn build_detail_dto(
    conn: &rusqlite::Connection,
    conv_id: &str,
    conv: ConversationRow,
    messages: Vec<MessageRow>,
    total_count: i64,
) -> Result<ConversationDetailDto, String> {
    let msg_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
    let sibling_counts = conversations::get_sibling_counts_batch(conn, conv_id, &msg_ids)?;
    let message_dtos: Vec<MessageDto> = messages
        .into_iter()
        .map(|m| {
            let mut dto = MessageDto::from(m);
            dto.sibling_count = sibling_counts.get(&dto.id).copied();
            dto
        })
        .collect();
    Ok(ConversationDetailDto {
        conversation: conv.into(),
        messages: message_dtos,
        total_count,
    })
}

fn get_conversation_detail_impl(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<ConversationDetailDto>, String> {
    let detail = conversations::get_conversation_detail(conn, id)?;
    match detail {
        Some(d) => {
            let total_count = d.messages.len() as i64;
            Ok(Some(build_detail_dto(
                conn,
                id,
                d.conversation,
                d.messages,
                total_count,
            )?))
        }
        None => Ok(None),
    }
}

fn get_conversation_paginated_impl(
    conn: &rusqlite::Connection,
    id: &str,
    limit: i64,
    before_timestamp: Option<i64>,
) -> Result<Option<ConversationDetailDto>, String> {
    match conversations::get_conversation_by_id(conn, id)? {
        Some(conv) => {
            let (messages, total_count) = if let Some(ref leaf_id) = conv.active_branch_id {
                let branch = conversations::trace_active_branch(conn, id, leaf_id)?;
                let count = conversations::get_message_count(conn, id)?;
                (branch, count)
            } else {
                let msgs =
                    conversations::get_messages_paginated(conn, id, limit, before_timestamp)?;
                let count = conversations::get_message_count(conn, id)?;
                (msgs, count)
            };
            Ok(Some(build_detail_dto(
                conn,
                id,
                conv,
                messages,
                total_count,
            )?))
        }
        None => Ok(None),
    }
}

fn get_conversation_all_impl(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<ConversationDetailDto>, String> {
    match conversations::get_conversation_by_id(conn, id)? {
        Some(conv) => {
            let messages = conversations::get_messages(conn, id)?;
            let total_count = messages.len() as i64;
            Ok(Some(build_detail_dto(
                conn,
                id,
                conv,
                messages,
                total_count,
            )?))
        }
        None => Ok(None),
    }
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)?;

    emit_list_changed(&app, ConversationListChangeKindDto::Deleted, Some(id), None);

    Ok(())
}

#[command]
pub fn patch_conversation(
    app: AppHandle,
    id: String,
    patch: ConversationPatchDto,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let mut changed_kinds: Vec<ConversationListChangeKindDto> = Vec::new();

    if let Some(ref title) = patch.title {
        let source = patch.title_source.as_deref().unwrap_or("user");
        conversations::rename_conversation(&conn, &id, title, source)?;
        changed_kinds.push(ConversationListChangeKindDto::Renamed);
    }

    if patch.model_id.is_some() || patch.provider_id.is_some() {
        let now = Utc::now().timestamp_millis();
        let mut conv = conversations::get_conversation_by_id(&conn, &id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;

        if let Some(ref model_id) = patch.model_id {
            conv.model_id = model_id.clone();
            changed_kinds.push(ConversationListChangeKindDto::ModelUpdated);
        }
        if let Some(ref provider_id) = patch.provider_id {
            conv.provider_id = Some(provider_id.clone());
            changed_kinds.push(ConversationListChangeKindDto::ProviderUpdated);
        }
        conv.updated_at = now;
        conversations::update_conversation(&conn, &conv)?;
    }

    for kind in changed_kinds {
        emit_list_changed(&app, kind, Some(id.clone()), None);
    }

    Ok(())
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
                    change_kind: MessageChangeKindDto::MessageDeleted,
                    messages: super::conversation_utils::messages_with_sibling_counts(
                        &conn,
                        cid,
                        &detail.messages,
                    )?,
                    active_branch_id: detail.conversation.active_branch_id.clone(),
                    seq: None,
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
pub fn export_conversation(
    app: AppHandle,
    id: String,
    format: String,
    path: Option<String>,
) -> Result<Option<String>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let content = match format.as_str() {
        "json" => conversations::export_conversation_json(&conn, &id)
            .map_err(super::translate_storage_error)?,
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id)
            .map_err(super::translate_storage_error)?,
        _ => conversations::export_conversation_markdown(&conn, &id)
            .map_err(super::translate_storage_error)?,
    };
    match path {
        Some(p) => {
            std::fs::write(&p, &content).map_err(|e| e.to_string())?;
            Ok(None)
        }
        None => Ok(Some(content)),
    }
}

#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let result = conversations::toggle_pin(&conn, &id)?;

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::PinToggled,
        Some(id),
        None,
    );

    Ok(result)
}

#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let new_id = conversations::duplicate_conversation(&conn, &id)
        .map_err(super::translate_storage_error)?;

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::Duplicated,
        Some(new_id.clone()),
        None,
    );

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

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::BatchDeleted,
        None,
        Some(affected),
    );

    Ok(())
}

#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::purge_all(&conn)?;

    emit_list_changed(&app, ConversationListChangeKindDto::Purged, None, None);

    Ok(())
}

#[command]
pub async fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<TokenUsageSummaryDto, String> {
    let (summary, detail, provider_row, window) = {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        let window = context_window_tokens.filter(|&c| c > 0).or_else(|| {
            super::conversation_utils::resolve_context_window_for_conversation(
                &conn,
                &conversation_id,
            )
        });
        let conversation = conversations::get_conversation_by_id(&conn, &conversation_id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
        let summary = assistant_storage::token_usage::get_summary(
            &conn,
            &conversation_id,
            conversation.active_branch_id.as_deref(),
            window,
        )?;
        let detail = conversations::get_conversation_detail(&conn, &conversation_id)?;
        let provider_row = conversation
            .provider_id
            .as_deref()
            .and_then(|id| provider_store::get_provider_by_id(&conn, id).ok().flatten());
        (summary, detail, provider_row, window)
    };

    let mut dto: TokenUsageSummaryDto = summary.into();

    // 记录表在该分支零记录时，按分支出站投影估算兜底；只读，不写记录表、不写 messages.tokens
    if dto.occupancy.used_tokens.is_none() {
        if let (Some(detail), Some(provider_row)) = (detail, provider_row) {
            if !detail.messages.is_empty() {
                let attachment_store = app.state::<AttachmentStore>();
                let estimated =
                    chat_request::estimate_branch_tokens(&detail, &provider_row, &attachment_store)
                        .await;
                if estimated > 0 {
                    let estimated = estimated as i64;
                    let window = window.filter(|&w| w > 0);
                    dto.occupancy.used_tokens = Some(estimated);
                    dto.occupancy.estimated = true;
                    dto.occupancy.estimated_source = Some(TokenUsageEstimatedSourceDto::Projection);
                    dto.occupancy.context_window_tokens = window;
                    dto.occupancy.percentage =
                        window.map(|w| (estimated as f32 / w as f32).clamp(0.0, 1.0));
                }
            }
        }
    }

    Ok(dto)
}

#[command]
pub fn create_branch(
    app: AppHandle,
    conversation_id: String,
    branch_point_message_id: String,
    new_role: String,
    new_content: String,
    seq: Option<u32>,
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
            change_kind: MessageChangeKindDto::BranchCreated,
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            seq,
        },
    );

    Ok(crate::protocol::MessageDto::from(new_msg))
}

#[command]
pub fn switch_branch(
    app: AppHandle,
    conversation_id: String,
    target_message_id: String,
    seq: Option<u32>,
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
            change_kind: MessageChangeKindDto::BranchSwitched,
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            seq,
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
    )
    .map_err(super::translate_storage_error)?;

    let detail = conversations::get_conversation_detail(&conn, &source_conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        crate::protocol::ChatMessagesChangedPayloadDto {
            conversation_id: source_conversation_id.clone(),
            change_kind: MessageChangeKindDto::BranchForked,
            messages: super::conversation_utils::messages_with_sibling_counts(
                &conn,
                &source_conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            seq: None,
        },
    );

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::Created,
        Some(new_conv_id.clone()),
        None,
    );

    Ok(new_conv_id)
}
