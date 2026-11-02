use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AgentModeRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub prompt: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub reasoning_enabled: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub tools_json: String,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

const MODE_SELECT: &str = "id, name, description, kind, prompt, model, temperature, max_tokens, reasoning_enabled, reasoning_effort, tools, presentation_rank, is_builtin, created_at, updated_at";

fn read_mode_row(row: &rusqlite::Row) -> rusqlite::Result<AgentModeRow> {
    Ok(AgentModeRow {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        kind: row.get(3)?,
        prompt: row.get(4)?,
        model: row.get(5)?,
        temperature: row.get(6)?,
        max_tokens: row.get(7)?,
        reasoning_enabled: row.get::<_, Option<i64>>(8)?.map(|v| v != 0),
        reasoning_effort: row.get(9)?,
        tools_json: row.get(10)?,
        presentation_rank: row.get::<_, i64>(11)? as u32,
        is_builtin: row.get::<_, i64>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

pub fn init_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_modes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'custom',
            prompt TEXT NOT NULL DEFAULT '',
            model TEXT,
            temperature REAL,
            max_tokens INTEGER,
            reasoning_enabled INTEGER,
            reasoning_effort TEXT,
            tools TEXT NOT NULL DEFAULT '[]',
            presentation_rank INTEGER NOT NULL DEFAULT 99,
            is_builtin INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn insert_mode(conn: &Connection, row: &AgentModeRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO agent_modes ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            MODE_SELECT
        ),
        params![
            row.id,
            row.name,
            row.description,
            row.kind,
            row.prompt,
            row.model,
            row.temperature,
            row.max_tokens,
            row.reasoning_enabled.map(|b| b as i64),
            row.reasoning_effort,
            row.tools_json,
            row.presentation_rank as i64,
            row.is_builtin as i64,
            row.created_at,
            row.updated_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_modes(conn: &Connection) -> Result<Vec<AgentModeRow>, String> {
    let sql = format!(
        "SELECT {} FROM agent_modes ORDER BY presentation_rank ASC, created_at ASC",
        MODE_SELECT
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], read_mode_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<AgentModeRow>, String> {
    let sql = format!("SELECT {} FROM agent_modes WHERE id = ?1", MODE_SELECT);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(read_mode_row(row).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

pub fn update_mode(conn: &Connection, row: &AgentModeRow) -> Result<(), String> {
    conn.execute(
        "UPDATE agent_modes SET name=?2, description=?3, kind=?4, prompt=?5, model=?6, temperature=?7, max_tokens=?8, reasoning_enabled=?9, reasoning_effort=?10, tools=?11, presentation_rank=?12, is_builtin=?13, updated_at=?14 WHERE id=?1",
        params![
            row.id,
            row.name,
            row.description,
            row.kind,
            row.prompt,
            row.model,
            row.temperature,
            row.max_tokens,
            row.reasoning_enabled.map(|b| b as i64),
            row.reasoning_effort,
            row.tools_json,
            row.presentation_rank as i64,
            row.is_builtin as i64,
            row.updated_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_mode(conn: &Connection, id: &str) -> Result<(), StorageError> {
    let mode = get_by_id(conn, id)?;
    if let Some(m) = &mode {
        if m.is_builtin {
            return Err(StorageError::BuiltinModeCannotDelete);
        }
    }
    conn.execute("DELETE FROM agent_modes WHERE id = ?1", params![id])
        .map_err(|e| StorageError::other(e.to_string()))?;
    Ok(())
}

fn builtin_defaults(now: i64) -> Vec<AgentModeRow> {
    vec![
        AgentModeRow {
            id: "builtin-default".to_string(),
            name: "agent_mode.builtin.default.name".to_string(),
            description: "agent_mode.builtin.default.description".to_string(),
            kind: "default".to_string(),
            prompt: String::new(),
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            reasoning_enabled: None,
            reasoning_effort: None,
            tools_json: "[]".to_string(),
            presentation_rank: 0,
            is_builtin: true,
            created_at: now,
            updated_at: now,
        },
        AgentModeRow {
            id: "builtin-quick".to_string(),
            name: "agent_mode.builtin.quick.name".to_string(),
            description: "agent_mode.builtin.quick.description".to_string(),
            kind: "quick".to_string(),
            prompt: String::new(),
            model: None,
            temperature: Some(0.3),
            max_tokens: Some(2048),
            reasoning_enabled: None,
            reasoning_effort: None,
            tools_json: "[]".to_string(),
            presentation_rank: 1,
            is_builtin: true,
            created_at: now,
            updated_at: now,
        },
        AgentModeRow {
            id: "builtin-deep".to_string(),
            name: "agent_mode.builtin.deep.name".to_string(),
            description: "agent_mode.builtin.deep.description".to_string(),
            kind: "deep".to_string(),
            prompt: String::new(),
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(8192),
            reasoning_enabled: Some(true),
            reasoning_effort: Some("high".to_string()),
            tools_json: "[]".to_string(),
            presentation_rank: 2,
            is_builtin: true,
            created_at: now,
            updated_at: now,
        },
        AgentModeRow {
            id: "builtin-translate".to_string(),
            name: "agent_mode.builtin.translate.name".to_string(),
            description: "agent_mode.builtin.translate.description".to_string(),
            kind: "custom".to_string(),
            prompt: "agent_mode.builtin.translate.prompt".to_string(),
            model: None,
            temperature: Some(0.3),
            max_tokens: Some(4096),
            reasoning_enabled: None,
            reasoning_effort: None,
            tools_json: "[]".to_string(),
            presentation_rank: 10,
            is_builtin: true,
            created_at: now,
            updated_at: now,
        },
    ]
}

pub fn seed_builtin_modes(conn: &Connection) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    for mode in &builtin_defaults(now) {
        insert_mode(conn, mode)?;
    }
    Ok(())
}

pub fn reset_builtin_mode(conn: &Connection, id: &str) -> Result<(), StorageError> {
    let now = chrono::Utc::now().timestamp_millis();
    let defaults = builtin_defaults(now);
    let default_mode = defaults
        .iter()
        .find(|m| m.id == id)
        .ok_or(StorageError::NotBuiltinModeCannotReset)?;

    conn.execute(
        "UPDATE agent_modes SET description=?2, prompt=?3, model=?4, temperature=?5, max_tokens=?6, reasoning_enabled=?7, reasoning_effort=?8, tools=?9, updated_at=?10 WHERE id=?1",
        params![
            id,
            default_mode.description,
            default_mode.prompt,
            default_mode.model,
            default_mode.temperature,
            default_mode.max_tokens,
            default_mode.reasoning_enabled.map(|b| b as i64),
            default_mode.reasoning_effort,
            default_mode.tools_json,
            now
        ],
    )
    .map_err(|e| StorageError::Other(e.to_string()))?;
    Ok(())
}
