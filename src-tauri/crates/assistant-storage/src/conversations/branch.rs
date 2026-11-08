use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::crud::get_conversation_by_id;
use super::msg_ops::get_message_by_id;
use super::{read_msg_row, MessageRow, MSG_SELECT};

/// 环检测已是主防线（`trace_active_branch` / `find_branch_leaf` 的 `visited` 集合），
/// 深度上限只作防环冗余，须高于正常会话的链长量级：一次 turn 每迭代写
/// assistant + tool 共 2 行，多工具批次链式追加后每轮 5–6 行，故上限取 10000。
const MAX_BRANCH_DEPTH: usize = 10000;
const MAX_SIBLINGS: i64 = 20;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SiblingInfo {
    pub id: String,
    pub sibling_index: i64,
    pub content_preview: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaxSiblingsExceededError {
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub current_count: i64,
    pub max_siblings: i64,
}

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
                "SELECT id, sibling_index, content FROM messages
                 WHERE conversation_id = ?1 AND parent_id = ?2
                 ORDER BY sibling_index",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![conv_id, pid], |row| {
                let id: String = row.get(0)?;
                let sibling_index: i64 = row.get(1)?;
                let content: String = row.get(2)?;
                Ok(SiblingInfo {
                    id,
                    sibling_index,
                    content_preview: crate::content_codec::content_text_preview(&content, 100),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, sibling_index, content FROM messages
                 WHERE conversation_id = ?1 AND parent_id IS NULL
                 ORDER BY sibling_index",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![conv_id], |row| {
                let id: String = row.get(0)?;
                let sibling_index: i64 = row.get(1)?;
                let content: String = row.get(2)?;
                Ok(SiblingInfo {
                    id,
                    sibling_index,
                    content_preview: crate::content_codec::content_text_preview(&content, 100),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    Ok(siblings)
}

/// 追加一行后把 `active_branch_id` 推进到新行。
///
/// 仅当新行的 `parent_id` 正是当前活动叶子（或会话尚无活动分支）时才推进：
/// 一轮内可能连续追加多行（agent 每次迭代一条 assistant + 若干 tool），
/// 每行都必须成为新的叶子，否则 `trace_active_branch` 只能追到旧叶子，
/// 同轮后续行会从详情里消失。若用户已切到别的分支（`parent_id` 与叶子不符），
/// 则不抢占其选择。
pub fn advance_active_branch(
    conn: &Connection,
    conversation_id: &str,
    parent_id: Option<&str>,
    new_id: &str,
) -> Result<(), String> {
    let active: Option<String> = conn
        .query_row(
            "SELECT active_branch_id FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    let extends_active = match (active.as_deref(), parent_id) {
        (None, _) => true,
        (Some(active_id), Some(parent)) => active_id == parent,
        (Some(_), None) => false,
    };

    if extends_active {
        update_active_branch_id(conn, conversation_id, new_id)?;
    }
    Ok(())
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

pub fn find_branch_leaf(conn: &Connection, message_id: &str) -> Result<String, String> {
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
