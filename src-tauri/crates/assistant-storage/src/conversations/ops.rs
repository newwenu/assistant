use rusqlite::{params, Connection};

use super::branch::trace_active_branch;
use super::crud::{get_conversation_by_id, insert_conversation};
use super::msg_ops::insert_message;
use super::{read_conv_row, ConversationRow, MessageRow};
use crate::attachment_store::duplicate_attachment_refs;

pub fn search_conversations(
    conn: &Connection,
    query: &str,
) -> Result<Vec<ConversationRow>, String> {
    let pattern = format!("%{}%", query);
    let sql = "SELECT DISTINCT c.id, c.title, c.title_source, c.provider_id, c.model_id, c.system_prompt, c.is_pinned, c.sort_order, c.generation, c.created_at, c.updated_at, c.is_full, c.active_branch_id \
         FROM conversations c \
         LEFT JOIN messages m ON m.conversation_id = c.id \
         WHERE c.title LIKE ?1 OR m.content LIKE ?1 \
         ORDER BY c.is_pinned DESC, c.sort_order DESC, c.updated_at DESC";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let candidates: Vec<ConversationRow> = stmt
        .query_map(params![pattern], read_conv_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let lower_query = query.to_ascii_lowercase();
    let mut results = Vec::new();
    for conv in candidates {
        let title_match = conv.title.to_ascii_lowercase().contains(&lower_query);
        if title_match {
            results.push(conv);
            continue;
        }
        let msg_sql = "SELECT content FROM messages WHERE conversation_id = ?1";
        let mut msg_stmt = conn.prepare(msg_sql).map_err(|e| e.to_string())?;
        let content_match: bool = msg_stmt
            .query_map(params![conv.id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .any(|c| {
                crate::content_codec::content_plain_text(&c)
                    .to_ascii_lowercase()
                    .contains(&lower_query)
            });
        if content_match {
            results.push(conv);
        }
    }
    Ok(results)
}

pub fn export_conversation_markdown(conn: &Connection, id: &str) -> Result<String, String> {
    let detail = super::get_conversation_detail(conn, id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;
    let mut md = format!("# {}\n\n", detail.conversation.title);
    for msg in &detail.messages {
        let plain = crate::content_codec::content_plain_text(&msg.content);
        match msg.role.as_str() {
            "user" => md.push_str(&format!("## User\n\n{}\n\n", plain)),
            "assistant" => md.push_str(&format!("## Assistant\n\n{}\n\n", plain)),
            "system" => md.push_str(&format!("## System\n\n{}\n\n", plain)),
            _ => md.push_str(&format!("## {}\n\n{}\n\n", msg.role, plain)),
        }
    }
    Ok(md)
}

pub fn export_conversation_json(conn: &Connection, id: &str) -> Result<String, String> {
    let detail = super::get_conversation_detail(conn, id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;
    serde_json::to_string_pretty(&detail).map_err(|e| e.to_string())
}

pub fn toggle_pin(conn: &Connection, id: &str) -> Result<bool, String> {
    let current: bool = conn
        .query_row(
            "SELECT is_pinned FROM conversations WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .map_err(|e| e.to_string())?;

    let new_pinned = !current;
    if new_pinned {
        let max_sort: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) FROM conversations WHERE is_pinned = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "UPDATE conversations SET is_pinned = 1, sort_order = ?2 WHERE id = ?1",
            params![id, max_sort + 1],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE conversations SET is_pinned = 0, sort_order = 0 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(new_pinned)
}

pub fn duplicate_conversation(conn: &Connection, id: &str) -> Result<String, String> {
    let detail = super::get_conversation_detail(conn, id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let new_conv = ConversationRow {
        id: new_id.clone(),
        title: format!(
            "{}{}",
            detail.conversation.title,
            assistant_i18n::t("conversation.copy_suffix")
        ),
        title_source: "user".to_string(),
        provider_id: detail.conversation.provider_id.clone(),
        model_id: detail.conversation.model_id.clone(),
        system_prompt: detail.conversation.system_prompt.clone(),
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: now,
        updated_at: now,
        is_full: false,
        active_branch_id: None,
    };
    insert_conversation(conn, &new_conv)?;

    let mut message_id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for msg in &detail.messages {
        let new_msg_id = uuid::Uuid::new_v4().to_string();
        message_id_map.insert(msg.id.clone(), new_msg_id.clone());
        let new_msg = MessageRow {
            id: new_msg_id,
            conversation_id: new_id.clone(),
            role: msg.role.clone(),
            content: msg.content.clone(),
            tokens: msg.tokens,
            timestamp: msg.timestamp,
            parent_id: None,
            is_pinned: msg.is_pinned,
            reasoning: msg.reasoning.clone(),
            tool_calls_json: msg.tool_calls_json.clone(),
            sibling_index: msg.sibling_index,
            provider_id: msg.provider_id.clone(),
            model_id: msg.model_id.clone(),
        };
        insert_message(conn, &new_msg)?;
    }

    duplicate_attachment_refs(conn, &id, &new_id, &message_id_map)?;

    Ok(new_id)
}

pub fn rename_conversation(
    conn: &Connection,
    id: &str,
    title: &str,
    title_source: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE conversations SET title = ?2, title_source = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, title, title_source, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn batch_delete(conn: &Connection, ids: &[String]) -> Result<(), String> {
    for id in ids {
        super::crud::delete_conversation(conn, id)?;
    }
    Ok(())
}

pub fn purge_all(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM attachment_refs", [])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM messages", [])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM conversations", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn increment_generation(conn: &Connection, id: &str) -> Result<i64, String> {
    conn.execute(
        "UPDATE conversations SET generation = generation + 1 WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    let gen: i64 = conn
        .query_row(
            "SELECT generation FROM conversations WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(gen)
}

pub fn fork_to_new_conversation(
    conn: &Connection,
    source_conversation_id: &str,
    branch_leaf_message_id: &str,
) -> Result<String, String> {
    let source_conv = get_conversation_by_id(conn, source_conversation_id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;

    let branch_messages =
        trace_active_branch(conn, source_conversation_id, branch_leaf_message_id)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let new_conv = ConversationRow {
        id: new_id.clone(),
        title: format!(
            "{}{}",
            source_conv.title,
            assistant_i18n::t("conversation.copy_suffix")
        ),
        title_source: "user".to_string(),
        provider_id: source_conv.provider_id.clone(),
        model_id: source_conv.model_id.clone(),
        system_prompt: source_conv.system_prompt.clone(),
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: now,
        updated_at: now,
        is_full: false,
        active_branch_id: None,
    };
    insert_conversation(conn, &new_conv)?;

    let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for msg in &branch_messages {
        let new_msg_id = uuid::Uuid::new_v4().to_string();
        id_map.insert(msg.id.clone(), new_msg_id.clone());
        let new_parent_id = msg
            .parent_id
            .as_ref()
            .and_then(|pid| id_map.get(pid))
            .cloned();
        let new_msg = MessageRow {
            id: new_msg_id,
            conversation_id: new_id.clone(),
            role: msg.role.clone(),
            content: msg.content.clone(),
            tokens: msg.tokens,
            timestamp: msg.timestamp,
            parent_id: new_parent_id,
            is_pinned: msg.is_pinned,
            reasoning: msg.reasoning.clone(),
            tool_calls_json: msg.tool_calls_json.clone(),
            sibling_index: 0,
            provider_id: msg.provider_id.clone(),
            model_id: msg.model_id.clone(),
        };
        insert_message(conn, &new_msg)?;
    }

    if let Some(last_msg) = branch_messages.last() {
        if let Some(last_new_id) = id_map.get(&last_msg.id) {
            conn.execute(
                "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
                params![last_new_id, new_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    duplicate_attachment_refs(conn, source_conversation_id, &new_id, &id_map)?;

    Ok(new_id)
}
