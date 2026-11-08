use rusqlite::{params, Connection, OptionalExtension};

use super::{read_msg_row, MessageRow, MSG_SELECT};

pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO messages ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            MSG_SELECT
        ),
        params![
            row.id,
            row.conversation_id,
            row.role,
            row.content,
            row.tokens,
            row.timestamp,
            row.parent_id,
            row.is_pinned as i64,
            row.reasoning,
            row.tool_calls_json,
            row.sibling_index,
            row.provider_id,
            row.model_id,
            row.tool_call_id,
            row.is_error as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_messages(conn: &Connection, conversation_id: &str) -> Result<Vec<MessageRow>, String> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
        MSG_SELECT
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], read_msg_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn update_message_content(
    conn: &Connection,
    id: &str,
    content: &str,
    tokens: Option<i64>,
    reasoning: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET content = ?2, tokens = ?3, reasoning = ?4 WHERE id = ?1",
        params![id, content, tokens, reasoning],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 写 `messages.tool_calls_json`；在 assistant 行 `finalize` 之前调用。
pub fn update_message_tool_calls(
    conn: &Connection,
    id: &str,
    tool_calls_json: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET tool_calls_json = ?2 WHERE id = ?1",
        params![id, tool_calls_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 写 `messages.status`；取值由调用方给出（`streaming` / `completed` / `interrupted` / `failed`）。
pub fn update_message_status(conn: &Connection, id: &str, status: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET status = ?2 WHERE id = ?1",
        params![id, status],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 启动清扫：上一进程未收尾的流式消息一律标记为 `interrupted`，返回受影响行数。
pub fn mark_streaming_messages_interrupted(conn: &Connection) -> Result<usize, String> {
    conn.execute(
        "UPDATE messages SET status = 'interrupted' WHERE status = 'streaming'",
        [],
    )
    .map_err(|e| e.to_string())
}

pub fn get_message_status(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT status FROM messages WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn get_messages_paginated(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
    before_timestamp: Option<i64>,
) -> Result<Vec<MessageRow>, String> {
    let rows: Vec<MessageRow> = if let Some(ts) = before_timestamp {
        let sql = format!("SELECT {} FROM messages WHERE conversation_id = ?1 AND timestamp < ?2 ORDER BY timestamp DESC LIMIT ?3", MSG_SELECT);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![conversation_id, ts, limit], read_msg_row)
            .map_err(|e| e.to_string())?;
        let mut r: Vec<MessageRow> = result
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        r.reverse();
        r
    } else {
        let sql = format!(
            "SELECT {} FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            MSG_SELECT
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![conversation_id, limit], read_msg_row)
            .map_err(|e| e.to_string())?;
        let mut r: Vec<MessageRow> = result
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        r.reverse();
        r
    };
    Ok(rows)
}

pub fn get_message_count(conn: &Connection, conversation_id: &str) -> Result<i64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

pub fn get_conversation_id_by_message(
    conn: &Connection,
    message_id: &str,
) -> Result<Option<String>, String> {
    let result = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn get_message_by_id(conn: &Connection, id: &str) -> Result<Option<MessageRow>, String> {
    let sql = format!("SELECT {} FROM messages WHERE id = ?1", MSG_SELECT);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(read_msg_row(row).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

pub fn get_tool_results(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<(String, String, bool)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT tool_call_id, content, is_error FROM messages WHERE conversation_id = ?1 AND role = 'tool' AND tool_call_id IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_sibling_count(
    conn: &Connection,
    conversation_id: &str,
    parent_id: Option<&str>,
) -> Result<i64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2",
            params![conversation_id, parent_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

pub fn get_sibling_counts_batch(
    conn: &Connection,
    conversation_id: &str,
    message_ids: &[String],
) -> Result<std::collections::HashMap<String, i64>, String> {
    let mut result = std::collections::HashMap::new();
    if message_ids.is_empty() {
        return Ok(result);
    }

    let mut parent_ids: std::collections::HashSet<Option<String>> =
        std::collections::HashSet::new();
    let mut stmt = conn
        .prepare("SELECT parent_id FROM messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    for mid in message_ids {
        let pid: Option<String> = stmt
            .query_row(params![mid], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        parent_ids.insert(pid);
    }

    let mut count_stmt = conn
        .prepare("SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2")
        .map_err(|e| e.to_string())?;
    let mut pid_counts: std::collections::HashMap<Option<String>, i64> =
        std::collections::HashMap::new();
    for pid in &parent_ids {
        let count: i64 = count_stmt
            .query_row(params![conversation_id, pid], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        pid_counts.insert(pid.clone(), count);
    }

    let mut pid_stmt = conn
        .prepare("SELECT parent_id FROM messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    for mid in message_ids {
        let pid: Option<String> = pid_stmt
            .query_row(params![mid], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        let count = pid_counts.get(&pid).copied().unwrap_or(1);
        result.insert(mid.clone(), count);
    }

    Ok(result)
}
