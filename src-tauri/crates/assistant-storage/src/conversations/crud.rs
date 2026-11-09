use rusqlite::{params, Connection};

use super::{read_conv_row, ConversationRow, CONV_SELECT};

pub fn insert_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO conversations ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
            row.active_branch_id,
            row.mode_id,
            row.tool_config_json,
            row.workspace_root
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

pub fn update_conversation(conn: &Connection, row: &ConversationRow) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET title=?2, title_source=?3, provider_id=?4, model_id=?5, system_prompt=?6, is_pinned=?7, sort_order=?8, generation=?9, created_at=?10, updated_at=?11, is_full=?12, active_branch_id=?13, mode_id=?14, tool_config_json=?15, workspace_root=?16 WHERE id=?1",
        params![row.id, row.title, row.title_source, row.provider_id, row.model_id, row.system_prompt, row.is_pinned as i64, row.sort_order, row.generation, row.created_at, row.updated_at, row.is_full as i64, row.active_branch_id, row.mode_id, row.tool_config_json, row.workspace_root],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_conversation(conn: &Connection, id: &str) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM conversations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM attachment_refs WHERE conversation_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    // 会话级授权随会话删除级联清理：`s:<conversationId>|<key>` 前缀由 Rust 侧唯一维护。
    let session_prefix = format!("s:{id}|");
    tx.execute(
        "DELETE FROM permission_grants WHERE instr(grant_key, ?1) = 1",
        params![session_prefix],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
