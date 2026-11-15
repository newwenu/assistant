//! `mcp_servers` 表：MCP server 的唯一运行时配置源（I38e §6.1）。
//!
//! 本模块只管「行」与「规范 JSON / 哈希」；加密在宿主层完成，明文配置不经过这里，
//! 也不写日志。`id` 是稳定标识（模型工具名与授权键都用它），创建后禁止修改。

use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// 一行 `mcp_servers`。`config_encrypted` 是密文的 base64 文本，`config_hash` 是明文哈希。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerRow {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub config_encrypted: String,
    pub config_hash: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 加密载荷的明文结构：`{command, args, cwd, env}`（I38e §6.1）。
///
/// 用 `BTreeMap` 让 `env` 天然有序，规范 JSON 与哈希因此与插入顺序无关。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpLaunchConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// 单次调用超时覆盖（毫秒）；缺省用客户端默认 30s（规格 §4.3）。
    /// `skip_serializing_if` 让「未配置」在规范 JSON 中不出现，哈希不受 `null` / 缺失影响。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_timeout_ms: Option<u64>,
}

impl McpLaunchConfig {
    /// 规范 JSON：对象键递归排序、无多余空白、数组顺序保留。
    ///
    /// 不依赖 `serde_json::Map` 的迭代顺序（该顺序受 feature 影响），
    /// 保证同一配置在不同构建 / 进程下得到同一哈希。
    pub fn canonical_json(&self) -> Result<String, String> {
        let value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        Ok(canonical_json(&value))
    }

    /// 明文规范 JSON 的 SHA-256 十六进制小写（I38e §6.2）：授权键失效判断的唯一输入。
    pub fn config_hash(&self) -> Result<String, String> {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_json()?.as_bytes());
        Ok(hex::encode(hasher.finalize()))
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    let encoded = serde_json::to_string(key).unwrap_or_default();
                    format!("{encoded}:{}", canonical_json(&map[key]))
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => {
            let encoded: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", encoded.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub fn insert_server(conn: &Connection, row: &McpServerRow) -> Result<(), String> {
    conn.execute(
        "INSERT INTO mcp_servers
           (id, name, transport, config_encrypted, config_hash, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.id,
            row.name,
            row.transport,
            row.config_encrypted,
            row.config_hash,
            row.enabled as i64,
            row.created_at,
            row.updated_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_servers(conn: &Connection) -> Result<Vec<McpServerRow>, String> {
    query_rows(conn, "WHERE 1 = 1", [])
}

pub fn list_enabled_servers(conn: &Connection) -> Result<Vec<McpServerRow>, String> {
    query_rows(conn, "WHERE enabled = 1", [])
}

fn query_rows<P: rusqlite::Params>(
    conn: &Connection,
    filter: &str,
    params: P,
) -> Result<Vec<McpServerRow>, String> {
    let sql = format!(
        "SELECT id, name, transport, config_encrypted, config_hash, enabled, created_at, updated_at
         FROM mcp_servers {filter} ORDER BY created_at, id"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params, map_row).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpServerRow> {
    Ok(McpServerRow {
        id: row.get(0)?,
        name: row.get(1)?,
        transport: row.get(2)?,
        config_encrypted: row.get(3)?,
        config_hash: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn get_server(conn: &Connection, id: &str) -> Result<Option<McpServerRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, transport, config_encrypted, config_hash, enabled, created_at, updated_at
             FROM mcp_servers WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(map_row(row).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

/// 更新可变字段；`id` 与 `created_at` 不在 SET 列表中，从 SQL 层面禁止改写。
pub fn update_server(conn: &Connection, row: &McpServerRow) -> Result<(), String> {
    conn.execute(
        "UPDATE mcp_servers
            SET name = ?2, transport = ?3, config_encrypted = ?4, config_hash = ?5,
                enabled = ?6, updated_at = ?7
          WHERE id = ?1",
        params![
            row.id,
            row.name,
            row.transport,
            row.config_encrypted,
            row.config_hash,
            row.enabled as i64,
            row.updated_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 启停只写 `enabled` 与 `updated_at`：运行时状态（Ready / Error / 重试次数）不落库。
pub fn set_server_enabled(
    conn: &Connection,
    id: &str,
    enabled: bool,
    updated_at: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE mcp_servers SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, enabled as i64, updated_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_server(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
