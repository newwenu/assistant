mod deleted;
mod field_crypto;
mod import;
mod query;
mod snapshot;
mod tx;
mod write;

use rusqlite::Connection;

use crate::types::VaultError;

pub(super) fn query_ids(conn: &Connection, sql: &str) -> Result<Vec<String>, VaultError> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| VaultError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| VaultError::Database(e.to_string()))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(ids)
}
