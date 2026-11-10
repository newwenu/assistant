use chrono::Utc;
use uuid::Uuid;

use assistant_storage::{
    attachment_store::{self},
    conversations::{self, MessageRow},
    providers::{self as provider_store},
};

pub(crate) struct InsertedMessages {
    pub assistant_msg_id: String,
    pub user_msg_id: Option<String>,
}

pub(crate) struct InsertChatInput<'a> {
    pub conversation_id: &'a str,
    pub content: &'a str,
    pub branch_point_message_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    /// 本次发送所选模式（I43 §3.4）；空串视为未选，回退 `conversations.mode_id`。
    pub mode_id: Option<&'a str>,
}

pub(crate) fn insert_chat_messages(
    conn: &rusqlite::Connection,
    input: InsertChatInput<'_>,
) -> Result<InsertedMessages, String> {
    let InsertChatInput {
        conversation_id,
        content,
        branch_point_message_id,
        provider_id,
        model_id,
        mode_id,
    } = input;

    if conversations::is_conversation_full(conn, conversation_id)
        .map_err(super::super::translate_storage_error)?
    {
        return Err("Conversation is full, cannot send".to_string());
    }

    let now = Utc::now().timestamp_millis();

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

    // 模式来源统一：本次所选（非空串）优先，否则回退会话现值（I43 §3.4）。
    let msg_mode_id: Option<String> = match mode_id {
        Some(raw) if !raw.is_empty() => Some(raw.to_string()),
        _ => detail_before.conversation.mode_id.clone(),
    };

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
            msg_mode_id.as_deref(),
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
            msg_mode_id.as_deref(),
        )?
    };

    conversations::update_active_branch_id(conn, conversation_id, &assistant_msg_id)?;

    Ok(InsertedMessages {
        assistant_msg_id,
        user_msg_id,
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
    mode_id: Option<&str>,
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
            mode_id,
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
            mode_id,
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
    mode_id: Option<&str>,
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
        mode_id,
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
            tool_call_id: None,
            is_error: false,
            status: "completed".to_string(),
            mode_id: None,
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
    mode_id: Option<&str>,
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
            tool_call_id: None,
            is_error: false,
            status: "streaming".to_string(),
            mode_id: mode_id.map(|s| s.to_string()),
        },
    )?;
    conversations::update_message_status(conn, &aid, "streaming")?;
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