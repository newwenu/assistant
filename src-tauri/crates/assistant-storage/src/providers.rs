use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub type_: String,
    pub base_url: String,
    pub api_key_encrypted: String,
    pub models_json: String,
    pub default_model: String,
    pub updated_at: i64,
}

pub fn insert_provider(conn: &Connection, row: &ProviderRow) -> Result<(), String> {
    conn.execute(
        "INSERT INTO providers (id, name, type, base_url, api_key_encrypted, models, default_model, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![row.id, row.name, row.type_, row.base_url, row.api_key_encrypted, row.models_json, row.default_model, row.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_providers(conn: &Connection) -> Result<Vec<ProviderRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, type, base_url, api_key_encrypted, models, default_model, updated_at FROM providers ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ProviderRow {
                id: row.get(0)?,
                name: row.get(1)?,
                type_: row.get(2)?,
                base_url: row.get(3)?,
                api_key_encrypted: row.get(4)?,
                models_json: row.get(5)?,
                default_model: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_provider_by_id(conn: &Connection, id: &str) -> Result<Option<ProviderRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, type, base_url, api_key_encrypted, models, default_model, updated_at FROM providers WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(ProviderRow {
            id: row.get(0).map_err(|e| e.to_string())?,
            name: row.get(1).map_err(|e| e.to_string())?,
            type_: row.get(2).map_err(|e| e.to_string())?,
            base_url: row.get(3).map_err(|e| e.to_string())?,
            api_key_encrypted: row.get(4).map_err(|e| e.to_string())?,
            models_json: row.get(5).map_err(|e| e.to_string())?,
            default_model: row.get(6).map_err(|e| e.to_string())?,
            updated_at: row.get(7).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_provider(conn: &Connection, row: &ProviderRow) -> Result<(), String> {
    conn.execute(
        "UPDATE providers SET name=?2, type=?3, base_url=?4, api_key_encrypted=?5, models=?6, default_model=?7, updated_at=?8 WHERE id=?1",
        params![row.id, row.name, row.type_, row.base_url, row.api_key_encrypted, row.models_json, row.default_model, row.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_provider(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET provider_id = NULL WHERE provider_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
