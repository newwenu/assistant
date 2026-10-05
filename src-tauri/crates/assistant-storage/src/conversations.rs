use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationDetail {
    pub conversation: ConversationRow,
    pub messages: Vec<MessageRow>,
}

pub fn insert_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        "INSERT INTO conversations (id, title, provider_id, model_id, system_prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![row.id, row.title, row.provider_id, row.model_id, row.system_prompt, row.created_at, row.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_conversations(
    conn: &Connection,
    provider_id: Option<&str>,
) -> Result<Vec<ConversationRow>, String> {
    let rows: Vec<ConversationRow>;
    if let Some(pid) = provider_id {
        let mut stmt = conn
            .prepare("SELECT id, title, provider_id, model_id, system_prompt, created_at, updated_at FROM conversations WHERE provider_id = ?1 ORDER BY updated_at DESC")
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![pid], |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_id: row.get(2)?,
                    model_id: row.get(3)?,
                    system_prompt: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows = result.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    } else {
        let mut stmt = conn
            .prepare("SELECT id, title, provider_id, model_id, system_prompt, created_at, updated_at FROM conversations ORDER BY updated_at DESC")
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map([], |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_id: row.get(2)?,
                    model_id: row.get(3)?,
                    system_prompt: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows = result.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }
    Ok(rows)
}

pub fn get_conversation_detail(
    conn: &Connection,
    id: &str,
) -> Result<Option<ConversationDetail>, String> {
    let conversation = {
        let mut stmt = conn
            .prepare("SELECT id, title, provider_id, model_id, system_prompt, created_at, updated_at FROM conversations WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Some(ConversationRow {
                id: row.get(0).map_err(|e| e.to_string())?,
                title: row.get(1).map_err(|e| e.to_string())?,
                provider_id: row.get(2).map_err(|e| e.to_string())?,
                model_id: row.get(3).map_err(|e| e.to_string())?,
                system_prompt: row.get(4).map_err(|e| e.to_string())?,
                created_at: row.get(5).map_err(|e| e.to_string())?,
                updated_at: row.get(6).map_err(|e| e.to_string())?,
            })
        } else {
            None
        }
    };
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
        "UPDATE conversations SET title=?2, provider_id=?3, model_id=?4, system_prompt=?5, created_at=?6, updated_at=?7 WHERE id=?1",
        params![row.id, row.title, row.provider_id, row.model_id, row.system_prompt, row.created_at, row.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_conversation(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), String> {
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, tokens, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![row.id, row.conversation_id, row.role, row.content, row.tokens, row.timestamp],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_messages(conn: &Connection, conversation_id: &str) -> Result<Vec<MessageRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, conversation_id, role, content, tokens, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tokens: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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
        let mut stmt = conn
            .prepare("SELECT id, conversation_id, role, content, tokens, timestamp FROM messages WHERE conversation_id = ?1 AND timestamp < ?2 ORDER BY timestamp DESC LIMIT ?3")
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![conversation_id, ts, limit], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    tokens: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut r: Vec<MessageRow> = result.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        r.reverse();
        r
    } else {
        let mut stmt = conn
            .prepare("SELECT id, conversation_id, role, content, tokens, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT ?2")
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![conversation_id, limit], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    tokens: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut r: Vec<MessageRow> = result.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
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

pub fn search_conversations(
    conn: &Connection,
    query: &str,
) -> Result<Vec<ConversationRow>, String> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT c.id, c.title, c.provider_id, c.model_id, c.system_prompt, c.created_at, c.updated_at \
             FROM conversations c \
             LEFT JOIN messages m ON m.conversation_id = c.id \
             WHERE c.title LIKE ?1 OR m.content LIKE ?1 \
             ORDER BY c.updated_at DESC"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern], |row| {
            Ok(ConversationRow {
                id: row.get(0)?,
                title: row.get(1)?,
                provider_id: row.get(2)?,
                model_id: row.get(3)?,
                system_prompt: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn export_conversation_markdown(
    conn: &Connection,
    id: &str,
) -> Result<String, String> {
    let detail = get_conversation_detail(conn, id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
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