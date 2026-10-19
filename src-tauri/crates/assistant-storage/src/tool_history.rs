use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const MAX_HISTORY_ENTRIES: i32 = 10;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolHistoryRow {
    pub tool_id: String,
    pub used_at: i64,
}

pub fn record_usage(conn: &Connection, tool_id: &str) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO tool_history (tool_id, used_at) VALUES (?1, ?2)",
        params![tool_id, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_recent_tools(conn: &Connection, limit: i32) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT tool_id FROM tool_history ORDER BY used_at DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn cleanup_old_history(conn: &Connection) -> Result<usize, String> {
    let affected = conn
        .execute(
            "DELETE FROM tool_history WHERE tool_id NOT IN (
                SELECT tool_id FROM tool_history ORDER BY used_at DESC LIMIT ?1
            )",
            params![MAX_HISTORY_ENTRIES],
        )
        .map_err(|e| e.to_string())?;
    Ok(affected)
}
