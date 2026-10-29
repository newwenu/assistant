use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use log::warn;
use rusqlite::{params, Connection, OptionalExtension};

use super::schema::SCHEMA_VERSION;

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

pub fn ensure_schema_version(conn: &Connection) -> Result<(), String> {
    let current: Option<String> = get_meta(conn, "schema_version")?;
    match current {
        Some(v) if v.parse::<u32>() == Ok(SCHEMA_VERSION) => Ok(()),
        Some(v) => {
            warn!(
                "Unsupported vault schema version: {} (expected {})",
                v, SCHEMA_VERSION
            );
            Err(format!(
                "Unsupported vault schema version: {} (expected {})",
                v, SCHEMA_VERSION
            ))
        }
        None => {
            set_meta(conn, "schema_version", &SCHEMA_VERSION.to_string())?;
            Ok(())
        }
    }
}

pub fn get_master_key_hash(conn: &Connection) -> Result<Option<Vec<u8>>, String> {
    let hash_b64 = get_meta(conn, "master_key_hash")?;
    match hash_b64 {
        Some(b64) => {
            let bytes = B64
                .decode(&b64)
                .map_err(|e| format!("Failed to decode master_key_hash: {}", e))?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

pub fn set_master_key_hash(conn: &Connection, hash: &[u8]) -> Result<(), String> {
    let b64 = B64.encode(hash);
    set_meta(conn, "master_key_hash", &b64)
}

pub fn get_salt(conn: &Connection, key: &str) -> Result<Option<[u8; 32]>, String> {
    let b64 = get_meta(conn, key)?;
    match b64 {
        Some(s) => {
            let bytes = B64
                .decode(&s)
                .map_err(|e| format!("Failed to decode salt '{}': {}", key, e))?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| format!("Salt '{}' must be 32 bytes", key))?;
            Ok(Some(arr))
        }
        None => Ok(None),
    }
}

pub fn set_salt(conn: &Connection, key: &str, salt: &[u8; 32]) -> Result<(), String> {
    let b64 = B64.encode(salt);
    set_meta(conn, key, &b64)
}
