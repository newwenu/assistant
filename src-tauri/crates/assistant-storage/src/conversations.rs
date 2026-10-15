use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const CONV_SELECT: &str = "id, title, title_source, provider_id, model_id, system_prompt, is_pinned, sort_order, generation, created_at, updated_at, is_full";
const MSG_SELECT: &str =
    "id, conversation_id, role, content, tokens, timestamp, parent_id, is_pinned";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub title_source: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub generation: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_full: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub is_pinned: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationDetail {
    pub conversation: ConversationRow,
    pub messages: Vec<MessageRow>,
}

fn read_conv_row(row: &rusqlite::Row) -> rusqlite::Result<ConversationRow> {
    Ok(ConversationRow {
        id: row.get(0)?,
        title: row.get(1)?,
        title_source: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        system_prompt: row.get(5)?,
        is_pinned: row.get::<_, i64>(6)? != 0,
        sort_order: row.get(7)?,
        generation: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        is_full: row.get::<_, i64>(11)? != 0,
    })
}

fn read_msg_row(row: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        tokens: row.get(4)?,
        timestamp: row.get(5)?,
        parent_id: row.get(6)?,
        is_pinned: row.get::<_, i64>(7)? != 0,
    })
}

pub fn insert_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO conversations ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            CONV_SELECT
        ),
        params![
            row.id,
            row.title,
            row.title_source,
            row.provider_id,
            row.model_id,
            row.system_prompt,
            row.is_pinned as i64,
            row.sort_order,
            row.generation,
            row.created_at,
            row.updated_at,
            row.is_full as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_conversations(
    conn: &Connection,
    provider_id: Option<&str>,
) -> Result<Vec<ConversationRow>, String> {
    if let Some(pid) = provider_id {
        let sql = format!("SELECT {} FROM conversations WHERE provider_id = ?1 ORDER BY is_pinned DESC, sort_order DESC, updated_at DESC", CONV_SELECT);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![pid], read_conv_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    } else {
        let sql = format!("SELECT {} FROM conversations ORDER BY is_pinned DESC, sort_order DESC, updated_at DESC", CONV_SELECT);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], read_conv_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

pub fn get_conversation_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<ConversationRow>, String> {
    let sql = format!("SELECT {} FROM conversations WHERE id = ?1", CONV_SELECT);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(read_conv_row(row).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

pub fn get_conversation_detail(
    conn: &Connection,
    id: &str,
) -> Result<Option<ConversationDetail>, String> {
    let conversation = get_conversation_by_id(conn, id)?;
    match conversation {
        Some(conv) => {
            let messages = get_messages(conn, id)?;
            Ok(Some(ConversationDetail {
                conversation: conv,
                messages,
            }))
        }
        None => Ok(None),
    }
}

pub fn update_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET title=?2, title_source=?3, provider_id=?4, model_id=?5, system_prompt=?6, is_pinned=?7, sort_order=?8, generation=?9, created_at=?10, updated_at=?11, is_full=?12 WHERE id=?1",
        params![row.id, row.title, row.title_source, row.provider_id, row.model_id, row.system_prompt, row.is_pinned as i64, row.sort_order, row.generation, row.created_at, row.updated_at, row.is_full as i64],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_conversation(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO messages ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
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
            row.is_pinned as i64
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
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET content = ?2, tokens = ?3 WHERE id = ?1",
        params![id, content, tokens],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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

pub fn delete_message(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM messages WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_message_and_after(conn: &Connection, id: &str) -> Result<(), String> {
    let row: (String, i64) = conn
        .query_row(
            "SELECT conversation_id, timestamp FROM messages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let (conversation_id, timestamp) = row;
    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1 AND (timestamp > ?2 OR id = ?3)",
        params![conversation_id, timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn search_conversations(
    conn: &Connection,
    query: &str,
) -> Result<Vec<ConversationRow>, String> {
    let pattern = format!("%{}%", query);
    let sql = "SELECT DISTINCT c.id, c.title, c.title_source, c.provider_id, c.model_id, c.system_prompt, c.is_pinned, c.sort_order, c.generation, c.created_at, c.updated_at, c.is_full \
         FROM conversations c \
         LEFT JOIN messages m ON m.conversation_id = c.id \
         WHERE c.title LIKE ?1 OR m.content LIKE ?1 \
         ORDER BY c.is_pinned DESC, c.sort_order DESC, c.updated_at DESC";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern], read_conv_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn export_conversation_markdown(conn: &Connection, id: &str) -> Result<String, String> {
    let detail =
        get_conversation_detail(conn, id)?.ok_or_else(|| "Conversation not found".to_string())?;
    let mut md = format!("# {}\n\n", detail.conversation.title);
    for msg in &detail.messages {
        match msg.role.as_str() {
            "user" => md.push_str(&format!("## User\n\n{}\n\n", msg.content)),
            "assistant" => md.push_str(&format!("## Assistant\n\n{}\n\n", msg.content)),
            "system" => md.push_str(&format!("## System\n\n{}\n\n", msg.content)),
            _ => md.push_str(&format!("## {}\n\n{}\n\n", msg.role, msg.content)),
        }
    }
    Ok(md)
}

pub fn export_conversation_json(conn: &Connection, id: &str) -> Result<String, String> {
    let detail =
        get_conversation_detail(conn, id)?.ok_or_else(|| "Conversation not found".to_string())?;
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
    let now = chrono::Utc::now().timestamp_millis();
    if new_pinned {
        let max_sort: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) FROM conversations WHERE is_pinned = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "UPDATE conversations SET is_pinned = 1, sort_order = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, max_sort + 1, now],
        ).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE conversations SET is_pinned = 0, sort_order = 0, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(new_pinned)
}

pub fn duplicate_conversation(conn: &Connection, id: &str) -> Result<String, String> {
    let detail =
        get_conversation_detail(conn, id)?.ok_or_else(|| "Conversation not found".to_string())?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let new_conv = ConversationRow {
        id: new_id.clone(),
        title: format!("{} (副本)", detail.conversation.title),
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
    };
    insert_conversation(conn, &new_conv)?;

    for msg in &detail.messages {
        let new_msg = MessageRow {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: new_id.clone(),
            role: msg.role.clone(),
            content: msg.content.clone(),
            tokens: msg.tokens,
            timestamp: msg.timestamp,
            parent_id: None,
            is_pinned: msg.is_pinned,
        };
        insert_message(conn, &new_msg)?;
    }
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
        delete_conversation(conn, id)?;
    }
    Ok(())
}

pub fn purge_all(conn: &Connection) -> Result<(), String> {
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

pub fn edit_message(conn: &Connection, id: &str, new_content: &str) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();

    let row: (String, i64) = conn
        .query_row(
            "SELECT conversation_id, timestamp FROM messages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let (conversation_id, timestamp) = row;

    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1 AND timestamp >= ?2 AND id != ?3",
        params![conversation_id, timestamp, id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE messages SET content = ?2, timestamp = ?3 WHERE id = ?1",
        params![id, new_content, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompressionStateRow {
    pub conversation_id: String,
    pub last_compression_at: Option<i64>,
    pub compression_count: i64,
}

fn read_compression_state_row(row: &rusqlite::Row) -> rusqlite::Result<CompressionStateRow> {
    Ok(CompressionStateRow {
        conversation_id: row.get(0)?,
        last_compression_at: row.get(1)?,
        compression_count: row.get(2)?,
    })
}

pub fn get_compression_state(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Option<CompressionStateRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, last_compression_at, compression_count FROM compression_state WHERE conversation_id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query(params![conversation_id])
        .map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(
            read_compression_state_row(row).map_err(|e| e.to_string())?,
        ))
    } else {
        Ok(None)
    }
}

pub fn upsert_compression_state(
    conn: &Connection,
    row: &CompressionStateRow,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO compression_state (conversation_id, last_compression_at, compression_count)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(conversation_id) DO UPDATE SET
           last_compression_at = excluded.last_compression_at,
           compression_count = excluded.compression_count",
        params![
            row.conversation_id,
            row.last_compression_at,
            row.compression_count,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_conversation_is_full(
    conn: &Connection,
    conversation_id: &str,
    is_full: bool,
) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET is_full = ?2 WHERE id = ?1",
        params![conversation_id, is_full as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_conversation_full(conn: &Connection, conversation_id: &str) -> Result<bool, String> {
    let result: Option<bool> = conn
        .query_row(
            "SELECT is_full FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .ok();
    result.ok_or_else(|| "Conversation not found".to_string())
}
