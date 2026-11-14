use rusqlite::{params, Connection};

use super::branch::trace_active_branch;
use super::crud::{get_conversation_by_id, insert_conversation};
use super::msg_ops::insert_message;
use super::{read_conv_row, ConversationRow, MessageRow};
use crate::attachment_store::duplicate_attachment_refs;
use crate::error::StorageError;

pub const COPY_TITLE_SOURCE: &str = "copy";

/// 复制 / 分叉会话时消息状态的承接：`streaming` 是运行期瞬时态，拷贝后不可能继续流式，
/// 一律落为 `completed`；工具占位行的在途态（`pending` / `running`）同理，副本里没有
/// 执行队列可推进，一律落为 `interrupted`；其余终态原样承接。
fn carried_status(status: &str) -> String {
    if status == "streaming" {
        "completed".to_string()
    } else if super::is_tool_row_in_flight(status) {
        "interrupted".to_string()
    } else {
        status.to_string()
    }
}

/// 复制 / 分叉时占位行的正文：副本不可能再等到结果，按原阶段补上对应文案，
/// 否则副本会留下一条正文为空的工具行。
fn carried_content(msg: &MessageRow) -> String {
    if msg.status == super::TOOL_ROW_PENDING {
        super::TOOL_NOT_STARTED_RESULT.to_string()
    } else if msg.status == super::TOOL_ROW_RUNNING {
        super::INTERRUPTED_TOOL_RESULT.to_string()
    } else {
        msg.content.clone()
    }
}

/// 让副本沿用源会话的 `next_turn_index`（I46 §2.4 路径 7）。
///
/// 副本已原样复制了源会话的 `turn_index`，计数器若从 1 重新开始就会与既有坐标撞号。
fn copy_turn_counter(conn: &Connection, source_id: &str, target_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET next_turn_index = \
            (SELECT next_turn_index FROM conversations WHERE id = ?1) WHERE id = ?2",
        params![source_id, target_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn search_conversations(
    conn: &Connection,
    query: &str,
) -> Result<Vec<ConversationRow>, String> {
    let pattern = format!("%{}%", query);
    let sql = "SELECT DISTINCT c.id, c.title, c.title_source, c.provider_id, c.model_id, c.system_prompt, c.is_pinned, c.sort_order, c.generation, c.created_at, c.updated_at, c.is_full, c.active_branch_id, c.mode_id, c.tool_config_json, c.workspace_root \
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

pub fn export_conversation_markdown(conn: &Connection, id: &str) -> Result<String, StorageError> {
    let detail =
        super::get_conversation_detail(conn, id)?.ok_or(StorageError::ConversationNotFound)?;
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

pub fn export_conversation_json(conn: &Connection, id: &str) -> Result<String, StorageError> {
    let detail =
        super::get_conversation_detail(conn, id)?.ok_or(StorageError::ConversationNotFound)?;
    serde_json::to_string_pretty(&detail).map_err(|e| StorageError::other(e.to_string()))
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

pub fn duplicate_conversation(conn: &Connection, id: &str) -> Result<String, StorageError> {
    let detail =
        super::get_conversation_detail(conn, id)?.ok_or(StorageError::ConversationNotFound)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let new_conv = ConversationRow {
        id: new_id.clone(),
        title: detail.conversation.title.clone(),
        title_source: COPY_TITLE_SOURCE.to_string(),
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
        mode_id: detail.conversation.mode_id.clone(),
        tool_config_json: detail.conversation.tool_config_json.clone(),
        workspace_root: detail.conversation.workspace_root.clone(),
    };
    insert_conversation(conn, &new_conv)?;

    // 坐标计数器沿用源会话（I46 §2.4 路径 7）：不重排、不重算，
    // 否则副本的下一个 turn 会与已复制的坐标撞号。
    copy_turn_counter(conn, id, &new_id)?;

    let mut message_id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for msg in &detail.messages {
        let new_msg_id = uuid::Uuid::new_v4().to_string();
        message_id_map.insert(msg.id.clone(), new_msg_id.clone());
        let new_msg = MessageRow {
            id: new_msg_id,
            conversation_id: new_id.clone(),
            role: msg.role.clone(),
            content: carried_content(msg),
            tokens: msg.tokens,
            timestamp: msg.timestamp,
            parent_id: None,
            is_pinned: msg.is_pinned,
            reasoning: msg.reasoning.clone(),
            tool_calls_json: msg.tool_calls_json.clone(),
            sibling_index: msg.sibling_index,
            provider_id: msg.provider_id.clone(),
            model_id: msg.model_id.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            is_error: msg.is_error || super::is_tool_row_in_flight(&msg.status),
            status: carried_status(&msg.status),
            mode_id: msg.mode_id.clone(),
            turn_index: msg.turn_index,
            iteration: msg.iteration,
        };
        insert_message(conn, &new_msg)?;
    }

    duplicate_attachment_refs(conn, id, &new_id, &message_id_map)?;

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
    // 会话全部清空后会话级授权全部失效；全局授权保留，直到用户撤销。
    conn.execute(
        "DELETE FROM permission_grants WHERE substr(grant_key, 1, 2) = 's:'",
        [],
    )
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
) -> Result<String, StorageError> {
    let source_conv = get_conversation_by_id(conn, source_conversation_id)?
        .ok_or(StorageError::ConversationNotFound)?;

    let branch_messages =
        trace_active_branch(conn, source_conversation_id, branch_leaf_message_id)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let new_conv = ConversationRow {
        id: new_id.clone(),
        title: source_conv.title.clone(),
        title_source: COPY_TITLE_SOURCE.to_string(),
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
        mode_id: source_conv.mode_id.clone(),
        tool_config_json: source_conv.tool_config_json.clone(),
        workspace_root: source_conv.workspace_root.clone(),
    };
    insert_conversation(conn, &new_conv)?;
    copy_turn_counter(conn, source_conversation_id, &new_id)?;

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
            content: carried_content(msg),
            tokens: msg.tokens,
            timestamp: msg.timestamp,
            parent_id: new_parent_id,
            is_pinned: msg.is_pinned,
            reasoning: msg.reasoning.clone(),
            tool_calls_json: msg.tool_calls_json.clone(),
            sibling_index: 0,
            provider_id: msg.provider_id.clone(),
            model_id: msg.model_id.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            is_error: msg.is_error || super::is_tool_row_in_flight(&msg.status),
            status: carried_status(&msg.status),
            mode_id: msg.mode_id.clone(),
            turn_index: msg.turn_index,
            iteration: msg.iteration,
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
