use chrono::Utc;
use uuid::Uuid;

use assistant_storage::{
    agent_modes::{self, AgentModeRow},
    attachment_store::{self},
    conversations::{self, MessageRow},
    providers::{self as provider_store},
};

pub(crate) struct InsertedMessages {
    pub mode: Option<AgentModeRow>,
    pub assistant_msg_id: String,
    pub user_msg_id: Option<String>,
    pub detail: conversations::ConversationDetail,
    pub provider_row: provider_store::ProviderRow,
}

pub(crate) fn insert_chat_messages(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    mode_id: Option<&str>,
    branch_point_message_id: Option<&str>,
    provider_id: &str,
    model_id: &str,
) -> Result<InsertedMessages, String> {
    if conversations::is_conversation_full(conn, conversation_id)
        .map_err(super::super::translate_storage_error)?
    {
        return Err("Conversation is full, cannot send".to_string());
    }

    let now = Utc::now().timestamp_millis();
    let mode: Option<AgentModeRow> =
        mode_id.and_then(|id| agent_modes::get_by_id(conn, id).ok().flatten());

    let detail_before = conversations::get_conversation_detail(conn, conversation_id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;

    provider_store::get_provider_by_id(conn, provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

    {
        let mut conv = conversations::get_conversation_by_id(conn, conversation_id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
        conv.provider_id = Some(provider_id.to_string());
        conv.model_id = model_id.to_string();
        conv.updated_at = now;
        conversations::update_conversation(conn, &conv)?;
    }

    let msg_provider_id = Some(provider_id.to_string());
    let msg_model_id = Some(model_id.to_string());

    let (_user_msg_id, assistant_msg_id, user_msg_id) = if let Some(bp_id) = branch_point_message_id
    {
        insert_on_branch(
            conn,
            conversation_id,
            content,
            bp_id,
            now,
            &msg_provider_id,
            &msg_model_id,
        )?
    } else {
        insert_normal_append(
            conn,
            conversation_id,
            content,
            &detail_before,
            now,
            &msg_provider_id,
            &msg_model_id,
        )?
    };

    conversations::update_active_branch_id(conn, conversation_id, &assistant_msg_id)?;

    let detail = conversations::get_conversation_detail(conn, conversation_id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    let provider_row = provider_store::get_provider_by_id(conn, provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

    Ok(InsertedMessages {
        mode,
        assistant_msg_id,
        user_msg_id,
        detail,
        provider_row,
    })
}

fn insert_on_branch(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    bp_id: &str,
    now: i64,
    msg_provider_id: &Option<String>,
    msg_model_id: &Option<String>,
) -> Result<(String, String, Option<String>), String> {
    let branch_point = conversations::get_message_by_id(conn, bp_id)?
        .ok_or_else(|| format!("Branch point message {} not found", bp_id))?;

    let bp_parent_id = branch_point.parent_id.clone();

    if branch_point.role == "user" {
        let user_parent_id = bp_parent_id.clone();
        let user_sibling_index =
            conversations::get_sibling_count(conn, conversation_id, user_parent_id.as_deref())?
                .max(0);

        let uid = insert_user_message_with_attachments(
            conn,
            conversation_id,
            content,
            user_parent_id,
            user_sibling_index,
            now,
        )?;

        let aid = insert_placeholder_assistant_message(
            conn,
            conversation_id,
            Some(uid.clone()),
            0,
            now + 1,
            msg_provider_id.as_deref(),
            msg_model_id.as_deref(),
        )?;

        Ok((uid.clone(), aid, Some(uid)))
    } else {
        let assistant_parent_id = bp_parent_id.clone();
        let assistant_sibling_index = conversations::get_sibling_count(
            conn,
            conversation_id,
            assistant_parent_id.as_deref(),
        )?
        .max(0);

        let aid = insert_placeholder_assistant_message(
            conn,
            conversation_id,
            assistant_parent_id,
            assistant_sibling_index,
            now,
            msg_provider_id.as_deref(),
            msg_model_id.as_deref(),
        )?;

        Ok((String::new(), aid, None))
    }
}

fn insert_normal_append(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    detail_before: &conversations::ConversationDetail,
    now: i64,
    msg_provider_id: &Option<String>,
    msg_model_id: &Option<String>,
) -> Result<(String, String, Option<String>), String> {
    let user_parent_id = detail_before.messages.last().map(|m| m.id.clone());
    let user_sibling_index =
        conversations::get_sibling_count(conn, conversation_id, user_parent_id.as_deref())?.max(0);

    let uid = insert_user_message_with_attachments(
        conn,
        conversation_id,
        content,
        user_parent_id,
        user_sibling_index,
        now,
    )?;

    let aid = insert_placeholder_assistant_message(
        conn,
        conversation_id,
        Some(uid.clone()),
        0,
        now + 1,
        msg_provider_id.as_deref(),
        msg_model_id.as_deref(),
    )?;

    Ok((uid.clone(), aid, Some(uid)))
}

fn insert_user_message_with_attachments(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    parent_id: Option<String>,
    sibling_index: i64,
    now: i64,
) -> Result<String, String> {
    let uid = Uuid::new_v4().to_string();
    conversations::insert_message(
        conn,
        &MessageRow {
            id: uid.clone(),
            conversation_id: conversation_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            tokens: None,
            timestamp: now,
            parent_id,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
            sibling_index,
            provider_id: None,
            model_id: None,
        },
    )?;

    {
        let attachment_ids = extract_attachment_ids(content);
        if !attachment_ids.is_empty() {
            attachment_store::insert_attachment_refs(conn, &attachment_ids, conversation_id, &uid)?;
        }
    }

    Ok(uid)
}

fn insert_placeholder_assistant_message(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    parent_id: Option<String>,
    sibling_index: i64,
    timestamp: i64,
    provider_id: Option<&str>,
    model_id: Option<&str>,
) -> Result<String, String> {
    let aid = Uuid::new_v4().to_string();
    conversations::insert_message(
        conn,
        &MessageRow {
            id: aid.clone(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: String::new(),
            tokens: None,
            timestamp,
            parent_id,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
            sibling_index,
            provider_id: provider_id.map(|s| s.to_string()),
            model_id: model_id.map(|s| s.to_string()),
        },
    )?;
    Ok(aid)
}

pub(crate) fn is_content_empty(content: &str) -> bool {
    assistant_storage::content_codec::is_content_empty(content)
}

pub(crate) fn extract_attachment_ids(content: &str) -> Vec<String> {
    let Some(blocks) = assistant_providers::types::parse_content(content) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            assistant_providers::types::ContentBlock::Image { attachment, .. } => {
                if attachment.attachment_id.is_empty() {
                    None
                } else {
                    Some(attachment.attachment_id.clone())
                }
            }
            assistant_providers::types::ContentBlock::File { attachment, .. } => {
                if attachment.attachment_id.is_empty() {
                    None
                } else {
                    Some(attachment.attachment_id.clone())
                }
            }
            _ => None,
        })
        .collect()
}
