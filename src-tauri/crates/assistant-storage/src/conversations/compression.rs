use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

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
