use rusqlite::{params, Connection, OptionalExtension};

use super::branch::find_branch_leaf;
use super::msg_ops::get_message_by_id;

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

pub(crate) fn compact_sibling_indices(
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

    let subtree_ids: Vec<String> = {
        let mut stmt = tx
            .prepare(
                "WITH RECURSIVE subtree(id) AS (
                    SELECT ?1
                    UNION ALL
                    SELECT m.id FROM messages m
                    JOIN subtree s ON m.parent_id = s.id
                    WHERE m.conversation_id = ?2
                )
                SELECT id FROM subtree",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![id, conv_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    for mid in &subtree_ids {
        tx.execute(
            "DELETE FROM attachment_refs WHERE message_id = ?1",
            params![mid],
        )
        .map_err(|e| e.to_string())?;
    }

    for mid in &subtree_ids {
        tx.execute("DELETE FROM messages WHERE id = ?1", params![mid])
            .map_err(|e| e.to_string())?;
    }

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
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();

        let needs_fix = match current_ab.as_ref() {
            None => true,
            Some(ab) => {
                let still_exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM messages WHERE id = ?1 AND conversation_id = ?2)",
                        params![ab, conv_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                !still_exists
            }
        };

        if needs_fix {
            let new_leaf = find_new_active_leaf(&tx, conv_id, parent_id.as_deref())?;

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
) -> Result<Option<String>, String> {
    if let Some(pid) = deleted_parent_id {
        let sibling_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2)",
                params![conv_id, pid],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if sibling_exists {
            let sibling_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2 ORDER BY sibling_index ASC LIMIT 1",
                    params![conv_id, pid],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            if let Some(sid) = sibling_id {
                return find_branch_leaf(conn, &sid).map(Some);
            }
        }
    }

    let any_msg: Option<String> = conn
        .query_row(
            "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 1",
            params![conv_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    match any_msg {
        Some(mid) => find_branch_leaf(conn, &mid).map(Some),
        None => Ok(None),
    }
}
