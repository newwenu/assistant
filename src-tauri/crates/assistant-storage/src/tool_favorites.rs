use rusqlite::{params, Connection};

pub fn get_favorites(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT tool_id FROM tool_favorites ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn toggle_favorite(conn: &Connection, tool_id: &str) -> Result<bool, String> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM tool_favorites WHERE tool_id = ?1",
            params![tool_id],
            |row| row.get::<_, i64>(0).map(|c| c > 0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "DELETE FROM tool_favorites WHERE tool_id = ?1",
            params![tool_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        conn.execute(
            "INSERT INTO tool_favorites (tool_id) VALUES (?1)",
            params![tool_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(true)
    }
}

pub fn set_favorites_order(conn: &Connection, tool_ids: &[String]) -> Result<(), String> {
    conn.execute("DELETE FROM tool_favorites", [])
        .map_err(|e| e.to_string())?;
    for (i, tool_id) in tool_ids.iter().enumerate() {
        conn.execute(
            "INSERT INTO tool_favorites (tool_id, rowid) VALUES (?1, ?2)",
            params![tool_id, (i + 1) as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}