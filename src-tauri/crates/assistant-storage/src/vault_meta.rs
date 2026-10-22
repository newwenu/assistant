use rusqlite::{params, Connection, OptionalExtension};

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let result = conn
        .query_row(
            "SELECT value FROM vault_meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO vault_meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_vault_initialized(conn: &Connection) -> Result<bool, String> {
    let hash = get_meta(conn, "master_key_hash")?;
    Ok(hash.is_some())
}
