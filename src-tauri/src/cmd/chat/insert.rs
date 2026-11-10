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

/// 写入 `assistant` 占位行的身份字段（I43 §3.4）：仅 assistant 行携带，`user` 行恒为 `NULL`。
struct MessageIdentity<'a> {
    provider_id: Option<&'a str>,
    model_id: Option<&'a str>,
    mode_id: Option<&'a str>,
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

    // 模式来源统一：本次所选（非空串）优先，否则回退会话现值（I43 §3.4）。
    let msg_mode_id: Option<String> = match mode_id {
        Some(raw) if !raw.is_empty() => Some(raw.to_string()),
        _ => detail_before.conversation.mode_id.clone(),
    };

    let identity = MessageIdentity {
        provider_id: Some(provider_id),
        model_id: Some(model_id),
        mode_id: msg_mode_id.as_deref(),
    };

    let (_user_msg_id, assistant_msg_id, user_msg_id) = if let Some(bp_id) = branch_point_message_id
    {
        insert_on_branch(conn, conversation_id, content, bp_id, now, &identity)?
    } else {
        insert_normal_append(
            conn,
            conversation_id,
            content,
            &detail_before,
            now,
            &identity,
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
    identity: &MessageIdentity<'_>,
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
            identity,
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
            identity,
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
    identity: &MessageIdentity<'_>,
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
        identity,
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
    identity: &MessageIdentity<'_>,
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
            provider_id: identity.provider_id.map(|s| s.to_string()),
            model_id: identity.model_id.map(|s| s.to_string()),
            tool_call_id: None,
            is_error: false,
            status: "streaming".to_string(),
            mode_id: identity.mode_id.map(|s| s.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_storage::conversations::ConversationRow;
    use assistant_storage::providers::ProviderRow;

    fn setup(conversation_mode: Option<&str>) -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = assistant_storage::db::init_db(&dir.path().join("chat.db")).unwrap();
        provider_store::insert_provider(
            &conn,
            &ProviderRow {
                id: "p1".to_string(),
                name: "p1".to_string(),
                type_: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_encrypted: String::new(),
                models_json: "[]".to_string(),
                default_model: "gpt-4".to_string(),
                updated_at: 1000,
            },
        )
        .unwrap();
        conversations::insert_conversation(
            &conn,
            &ConversationRow {
                id: "c1".to_string(),
                title: "test".to_string(),
                title_source: "placeholder".to_string(),
                provider_id: None,
                model_id: "gpt-4".to_string(),
                system_prompt: None,
                is_pinned: false,
                sort_order: 0,
                generation: 0,
                created_at: 1000,
                updated_at: 1000,
                is_full: false,
                active_branch_id: None,
                mode_id: conversation_mode.map(str::to_string),
                tool_config_json: None,
                workspace_root: String::new(),
            },
        )
        .unwrap();
        (dir, conn)
    }

    fn send(
        conn: &rusqlite::Connection,
        mode_id: Option<&str>,
        bp: Option<&str>,
    ) -> InsertedMessages {
        insert_chat_messages(
            conn,
            InsertChatInput {
                conversation_id: "c1",
                content: "hello",
                branch_point_message_id: bp,
                provider_id: "p1",
                model_id: "gpt-4",
                mode_id,
            },
        )
        .unwrap()
    }

    fn mode_of(conn: &rusqlite::Connection, id: &str) -> Option<String> {
        conversations::get_message_by_id(conn, id)
            .unwrap()
            .expect("message exists")
            .mode_id
    }

    #[test]
    fn chat_insert_writes_mode_only_to_assistant_row() {
        let (_dir, conn) = setup(None);
        let inserted = send(&conn, Some("builtin-agent"), None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-agent"),
            "A6: assistant 占位行写本次发送模式"
        );
        let user_id = inserted.user_msg_id.expect("普通追加必须建 user 行");
        assert_eq!(
            mode_of(&conn, &user_id),
            None,
            "A6: user 行 mode_id 为 NULL"
        );
    }

    #[test]
    fn empty_mode_falls_back_to_conversation_mode() {
        let (_dir, conn) = setup(Some("builtin-translate"));
        let inserted = send(&conn, Some(""), None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-translate"),
            "空串视为未选，回退 conversations.mode_id"
        );
    }

    #[test]
    fn missing_mode_falls_back_to_conversation_mode() {
        let (_dir, conn) = setup(Some("builtin-agent"));
        let inserted = send(&conn, None, None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-agent"),
            "缺省同样回退 conversations.mode_id"
        );
    }

    #[test]
    fn branch_insert_on_assistant_point_keeps_mode() {
        let (_dir, conn) = setup(Some("builtin-agent"));
        let first = send(&conn, Some("builtin-agent"), None);

        let second = send(
            &conn,
            Some("builtin-translate"),
            Some(&first.assistant_msg_id),
        );

        assert_eq!(
            mode_of(&conn, &second.assistant_msg_id).as_deref(),
            Some("builtin-translate"),
            "分支插入的 assistant 行取本次发送模式"
        );
        assert!(
            second.user_msg_id.is_none(),
            "以 assistant 为分支点时不新建 user 行"
        );
    }
}
