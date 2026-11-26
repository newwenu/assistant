use tauri::{command, AppHandle, Manager};

use super::to_dto;
use crate::cmd::{get_db_conn, translate_storage_error};
use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ChatMessagesChangedPayloadDto, ConversationListChangeKindDto, MessageChangeKindDto, MessageDto,
    SiblingInfoDto,
};
use assistant_storage::conversations;

use super::emit_list_changed;

#[command]
pub fn create_branch(
    app: AppHandle,
    conversation_id: String,
    branch_point_message_id: String,
    new_role: String,
    new_content: String,
    seq: Option<u32>,
) -> Result<MessageDto, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;

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
        ChatMessagesChangedPayloadDto {
            conversation_id: conversation_id.clone(),
            change_kind: MessageChangeKindDto::BranchCreated,
            messages: to_dto::messages_with_sibling_counts(
                &conn,
                &conversation_id,
                &detail.messages,
            )?,
            active_branch_id: detail.conversation.active_branch_id.clone(),
            seq,
        },
    );

    Ok(MessageDto::from(new_msg))
}

#[command]
pub fn switch_branch(
    app: AppHandle,
    conversation_id: String,
    target_message_id: String,
    seq: Option<u32>,
) -> Result<(), String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;

    conversations::switch_branch(&conn, &conversation_id, &target_message_id)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        ChatMessagesChangedPayloadDto {
            conversation_id: conversation_id.clone(),
            change_kind: MessageChangeKindDto::BranchSwitched,
            messages: to_dto::messages_with_sibling_counts(
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
) -> Result<Vec<SiblingInfoDto>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;

    let siblings = conversations::get_message_siblings(&conn, &message_id)?;
    Ok(siblings.into_iter().map(SiblingInfoDto::from).collect())
}

#[command]
pub fn increment_generation(app: AppHandle, id: String) -> Result<i64, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::increment_generation(&conn, &id)
}

#[command]
pub fn fork_to_new_conversation(
    app: AppHandle,
    source_conversation_id: String,
    branch_leaf_message_id: String,
) -> Result<String, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;

    let new_conv_id = conversations::fork_to_new_conversation(
        &conn,
        &source_conversation_id,
        &branch_leaf_message_id,
    )
    .map_err(translate_storage_error)?;

    let detail = conversations::get_conversation_detail(&conn, &source_conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    emit_broadcast(
        &app,
        event_name::CHAT_MESSAGES_CHANGED,
        ChatMessagesChangedPayloadDto {
            conversation_id: source_conversation_id.clone(),
            change_kind: MessageChangeKindDto::BranchForked,
            messages: to_dto::messages_with_sibling_counts(
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
