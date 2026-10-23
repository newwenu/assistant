use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use crate::content_codec;

const CONV_SELECT: &str = "id, title, title_source, provider_id, model_id, system_prompt, is_pinned, sort_order, generation, created_at, updated_at, is_full, active_branch_id";
const MSG_SELECT: &str =
    "id, conversation_id, role, content, tokens, timestamp, parent_id, is_pinned, reasoning, tool_calls_json, sibling_index, provider_id, model_id";

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
    pub active_branch_id: Option<String>,
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
    pub reasoning: Option<String>,
    pub tool_calls_json: Option<String>,
    pub sibling_index: i64,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
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
        active_branch_id: row.get(12)?,
    })
}

fn read_msg_row(row: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: row.get("id")?,
        conversation_id: row.get("conversation_id")?,
        role: row.get("role")?,
        content: row.get("content")?,
        tokens: row.get("tokens")?,
        timestamp: row.get("timestamp")?,
        parent_id: row.get("parent_id")?,
        is_pinned: row.get::<_, i64>("is_pinned")? != 0,
        reasoning: row.get("reasoning")?,
        tool_calls_json: row.get("tool_calls_json")?,
        sibling_index: row.get("sibling_index")?,
        provider_id: row.get("provider_id")?,
        model_id: row.get("model_id")?,
    })
}

pub fn insert_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO conversations ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
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
            row.is_full as i64,
            row.active_branch_id
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
            let messages = if let Some(ref leaf_id) = conv.active_branch_id {
                trace_active_branch(conn, id, leaf_id)?
            } else {
                get_messages(conn, id)?
            };
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
        "UPDATE conversations SET title=?2, title_source=?3, provider_id=?4, model_id=?5, system_prompt=?6, is_pinned=?7, sort_order=?8, generation=?9, created_at=?10, updated_at=?11, is_full=?12, active_branch_id=?13 WHERE id=?1",
        params![row.id, row.title, row.title_source, row.provider_id, row.model_id, row.system_prompt, row.is_pinned as i64, row.sort_order, row.generation, row.created_at, row.updated_at, row.is_full as i64, row.active_branch_id],
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
            "INSERT INTO messages ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
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
            row.model_id
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
    tool_calls_json: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET content = ?2, tokens = ?3, reasoning = ?4, tool_calls_json = ?5 WHERE id = ?1",
        params![id, content, tokens, reasoning, tool_calls_json],
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

pub fn get_subtree_size(conn: &Connection, id: &str) -> Result<i64, String> {
    let msg = get_message_by_id(conn, id)?.ok_or_else(|| format!("Message {} not found", id))?;
    let conv_id = &msg.conversation_id;

    let count: i64 = conn
        .query_row(
            "WITH RECURSIVE subtree(id) AS (
                SELECT ?1
                UNION ALL
                SELECT m.id FROM messages m
                JOIN subtree s ON m.parent_id = s.id
                WHERE m.conversation_id = ?2
            )
            SELECT COUNT(*) FROM subtree",
            params![id, conv_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

fn compact_sibling_indices(
    tx: &rusqlite::Transaction,
    conv_id: &str,
    parent_id: Option<&str>,
) -> Result<(), String> {
    let mut stmt = tx
        .prepare(
            "SELECT id FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2 ORDER BY sibling_index ASC, timestamp ASC",
        )
        .map_err(|e| e.to_string())?;

    let ids: Vec<String> = stmt
        .query_map(params![conv_id, parent_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    let mut update_stmt = tx
        .prepare("UPDATE messages SET sibling_index = ?1 WHERE id = ?2")
        .map_err(|e| e.to_string())?;

    for (i, mid) in ids.iter().enumerate() {
        update_stmt
            .execute(params![i as i64, mid])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_subtree(conn: &Connection, id: &str) -> Result<(), String> {
    let msg = get_message_by_id(conn, id)?.ok_or_else(|| format!("Message {} not found", id))?;
    let conv_id = &msg.conversation_id;
    let parent_id = msg.parent_id.clone();

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "WITH RECURSIVE subtree(id) AS (
            SELECT ?1
            UNION ALL
            SELECT m.id FROM messages m
            JOIN subtree s ON m.parent_id = s.id
            WHERE m.conversation_id = ?2
        )
        DELETE FROM messages WHERE id IN (SELECT id FROM subtree)",
        params![id, conv_id],
    )
    .map_err(|e| e.to_string())?;

    compact_sibling_indices(&tx, conv_id, parent_id.as_deref())?;

    let remaining: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ?1)",
            params![conv_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if remaining {
        let current_ab: Option<String> = tx
            .query_row(
                "SELECT active_branch_id FROM conversations WHERE id = ?1",
                params![conv_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        let needs_fix = current_ab.as_ref().map_or(true, |ab| {
            let still_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM messages WHERE id = ?1 AND conversation_id = ?2)",
                    params![ab, conv_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            !still_exists
        });

        if needs_fix {
            let new_leaf = find_new_active_leaf(&tx, conv_id, parent_id.as_deref());

            let now = chrono::Utc::now().timestamp_millis();
            tx.execute(
                "UPDATE conversations SET active_branch_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![new_leaf, now, conv_id],
            )
            .map_err(|e| e.to_string())?;
        }
    } else {
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute(
            "UPDATE conversations SET active_branch_id = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, conv_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn find_new_active_leaf(
    conn: &Connection,
    conv_id: &str,
    deleted_parent_id: Option<&str>,
) -> Option<String> {
    if let Some(pid) = deleted_parent_id {
        let sibling_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2)",
                params![conv_id, pid],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if sibling_exists {
            let sibling_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2 ORDER BY sibling_index ASC LIMIT 1",
                    params![conv_id, pid],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            if let Some(sid) = sibling_id {
                return find_branch_leaf(conn, &sid).ok();
            }
        }
    }

    let any_msg: Option<String> = conn
        .query_row(
            "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 1",
            params![conv_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    match any_msg {
        Some(mid) => find_branch_leaf(conn, &mid).ok(),
        None => None,
    }
}

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
            .any(|c| content_codec::content_plain_text(&c).to_ascii_lowercase().contains(&lower_query));
        if content_match {
            results.push(conv);
        }
    }
    Ok(results)
}

pub fn export_conversation_markdown(conn: &Connection, id: &str) -> Result<String, String> {
    let detail = get_conversation_detail(conn, id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;
    let mut md = format!("# {}\n\n", detail.conversation.title);
    for msg in &detail.messages {
        let plain = content_codec::content_plain_text(&msg.content);
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
    let detail = get_conversation_detail(conn, id)?
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
    let detail = get_conversation_detail(conn, id)?
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
            reasoning: msg.reasoning.clone(),
            tool_calls_json: msg.tool_calls_json.clone(),
            sibling_index: msg.sibling_index,
            provider_id: msg.provider_id.clone(),
            model_id: msg.model_id.clone(),
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
    result.ok_or_else(|| assistant_i18n::t("conversation.not_found"))
}

const MAX_BRANCH_DEPTH: usize = 1000;

pub fn trace_active_branch(
    conn: &Connection,
    _conversation_id: &str,
    leaf_id: &str,
) -> Result<Vec<MessageRow>, String> {
    let mut path: Vec<MessageRow> = Vec::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut current_id = Some(leaf_id.to_string());

    while let Some(cid) = current_id {
        if !visited.insert(cid.clone()) {
            return Err(format!("Cycle detected in parent chain at message {}", cid));
        }
        if path.len() >= MAX_BRANCH_DEPTH {
            return Err(format!(
                "Branch depth exceeds {} — possible data corruption",
                MAX_BRANCH_DEPTH
            ));
        }

        let sql = format!("SELECT {} FROM messages WHERE id = ?1", MSG_SELECT);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![cid]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let msg = read_msg_row(row).map_err(|e| e.to_string())?;
            current_id = msg.parent_id.clone();
            path.push(msg);
        } else {
            break;
        }
    }

    path.reverse();
    Ok(path)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SiblingInfo {
    pub id: String,
    pub sibling_index: i64,
    pub content_preview: String,
}

pub fn get_message_siblings(
    conn: &Connection,
    message_id: &str,
) -> Result<Vec<SiblingInfo>, String> {
    let parent_id: Option<String> = conn
        .query_row(
            "SELECT parent_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e: rusqlite::Error| e.to_string())?
        .flatten();

    let conv_id: String = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e: rusqlite::Error| e.to_string())?
        .ok_or_else(|| format!("Message {} not found", message_id))?;

    let siblings = if let Some(pid) = parent_id {
        let mut stmt = conn
            .prepare(
                "SELECT id, sibling_index, SUBSTR(content, 1, 100) FROM messages
                 WHERE conversation_id = ?1 AND parent_id = ?2
                 ORDER BY sibling_index",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![conv_id, pid], |row| {
                Ok(SiblingInfo {
                    id: row.get(0)?,
                    sibling_index: row.get(1)?,
                    content_preview: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, sibling_index, SUBSTR(content, 1, 100) FROM messages
                 WHERE conversation_id = ?1 AND parent_id IS NULL
                 ORDER BY sibling_index",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![conv_id], |row| {
                Ok(SiblingInfo {
                    id: row.get(0)?,
                    sibling_index: row.get(1)?,
                    content_preview: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    Ok(siblings)
}

const MAX_SIBLINGS: i64 = 20;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaxSiblingsExceededError {
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub current_count: i64,
    pub max_siblings: i64,
}

pub fn create_branch(
    conn: &Connection,
    conversation_id: &str,
    branch_point_message_id: &str,
    new_role: &str,
    new_content: &str,
) -> Result<MessageRow, String> {
    let branch_point = get_message_by_id(conn, branch_point_message_id)?
        .ok_or_else(|| format!("Message {} not found", branch_point_message_id))?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let sibling_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2",
            params![conversation_id, branch_point.parent_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if sibling_count >= MAX_SIBLINGS {
        tx.rollback().map_err(|e| e.to_string())?;
        return Err(format!(
            "MaxSiblingsExceeded: conversation={} parent={:?} count={} max={}",
            conversation_id, branch_point.parent_id, sibling_count, MAX_SIBLINGS
        ));
    }

    let max_sibling_index: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sibling_index), -1) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2",
            params![conversation_id, branch_point.parent_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to query max sibling_index: {}", e))?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let (provider_id, model_id): (Option<String>, Option<String>) = if new_role == "assistant" {
        let conv = get_conversation_by_id(&tx, conversation_id)?;
        match conv {
            Some(c) => (c.provider_id.clone(), Some(c.model_id.clone())),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    tx.execute(
        "INSERT INTO messages (id, conversation_id, role, content, tokens, timestamp, parent_id, is_pinned, reasoning, tool_calls_json, sibling_index, provider_id, model_id)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, 0, NULL, NULL, ?7, ?8, ?9)",
        params![
            new_id,
            conversation_id,
            new_role,
            new_content,
            now,
            branch_point.parent_id,
            max_sibling_index + 1,
            provider_id,
            model_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE conversations SET active_branch_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_id, now, conversation_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    get_message_by_id(conn, &new_id)?.ok_or_else(|| "Created message not found".to_string())
}

pub fn update_active_branch_id(
    conn: &Connection,
    conversation_id: &str,
    active_branch_id: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
        params![active_branch_id, conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn switch_branch(
    conn: &Connection,
    conversation_id: &str,
    target_message_id: &str,
) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM messages WHERE id = ?1 AND conversation_id = ?2",
            params![target_message_id, conversation_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if !exists {
        return Err(format!(
            "Message {} not found in conversation {}",
            target_message_id, conversation_id
        ));
    }

    let leaf_id = find_branch_leaf(conn, target_message_id)?;

    conn.execute(
        "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
        params![leaf_id, conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn find_branch_leaf(conn: &Connection, message_id: &str) -> Result<String, String> {
    let mut current = message_id.to_string();
    let mut visited = std::collections::HashSet::new();
    visited.insert(current.clone());

    loop {
        let child: Option<String> = conn
            .query_row(
                "SELECT id FROM messages WHERE parent_id = ?1 ORDER BY sibling_index DESC LIMIT 1",
                params![current],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e: rusqlite::Error| e.to_string())?
            .flatten();

        match child {
            Some(cid) => {
                if !visited.insert(cid.clone()) {
                    return Err(format!("Cycle detected in child chain at message {}", cid));
                }
                if visited.len() > MAX_BRANCH_DEPTH {
                    return Err(format!(
                        "Branch depth exceeds {} — possible data corruption",
                        MAX_BRANCH_DEPTH
                    ));
                }
                current = cid;
            }
            None => return Ok(current),
        }
    }
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

    Ok(new_id)
}