use rusqlite::{params, Connection};

/// 工作区未配置的哨兵值（I38h §1）：空串是合法状态，**不占用 `NULL`**。
pub const WORKSPACE_UNCONFIGURED: &str = "";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub path: String,
    pub name: String,
    pub created_at: i64,
    pub used_at: i64,
}

/// 目录簿列表，按最近使用倒序（I38h §2.2 / §8.3）。
pub fn list(conn: &Connection) -> Result<Vec<WorkspaceEntry>, String> {
    let mut stmt = conn
        .prepare("SELECT path, name, created_at, used_at FROM workspaces ORDER BY used_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(WorkspaceEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                used_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// 目录显示名缺省取 basename；根目录等无 basename 的路径退回路径本身。
pub fn default_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| path.to_string())
}

/// 登记或刷新一个工作区（仅由用户显式选择目录时调用）。
///
/// `name` 为 `None` 或空白时保留既有显示名，首次登记则取 basename。
pub fn upsert(conn: &Connection, path: &str, name: Option<&str>) -> Result<WorkspaceEntry, String> {
    let now = chrono::Utc::now().timestamp_millis();
    let trimmed = name.map(str::trim).filter(|n| !n.is_empty());

    let existing: Option<WorkspaceEntry> = {
        let mut stmt = conn
            .prepare("SELECT path, name, created_at, used_at FROM workspaces WHERE path = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![path]).map_err(|e| e.to_string())?;
        match rows.next().map_err(|e| e.to_string())? {
            Some(row) => Some(WorkspaceEntry {
                path: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                created_at: row.get(2).map_err(|e| e.to_string())?,
                used_at: row.get(3).map_err(|e| e.to_string())?,
            }),
            None => None,
        }
    };

    let display = match (trimmed, existing.as_ref()) {
        (Some(n), _) => n.to_string(),
        (None, Some(e)) => e.name.clone(),
        (None, None) => default_name(path),
    };

    match existing {
        Some(e) => {
            conn.execute(
                "UPDATE workspaces SET name = ?2, used_at = ?3 WHERE path = ?1",
                params![path, display, now],
            )
            .map_err(|err| err.to_string())?;
            Ok(WorkspaceEntry {
                path: path.to_string(),
                name: display,
                created_at: e.created_at,
                used_at: now,
            })
        }
        None => {
            conn.execute(
                "INSERT INTO workspaces (path, name, created_at, used_at) VALUES (?1, ?2, ?3, ?4)",
                params![path, display, now, now],
            )
            .map_err(|err| err.to_string())?;
            Ok(WorkspaceEntry {
                path: path.to_string(),
                name: display,
                created_at: now,
                used_at: now,
            })
        }
    }
}

/// 只删簿行；**不触碰** `conversations`（I38h §2.2 / A11）。
pub fn forget(conn: &Connection, path: &str) -> Result<(), String> {
    conn.execute("DELETE FROM workspaces WHERE path = ?1", params![path])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_name(conn: &Connection, path: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM workspaces WHERE path = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![path]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(row.get(0).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

/// 会话工作区绑定值的唯一写入点（I38h §2.1）。
///
/// `value` 只允许空串（未配置）或规范化绝对路径；调用方负责校验。
/// 与授权清理的原子性由调用方在更外层事务中保证（I38h §6.2）。
pub fn set_conversation_workspace(
    conn: &Connection,
    conversation_id: &str,
    value: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE conversations SET workspace_root = ?2, updated_at = ?3 WHERE id = ?1",
        params![conversation_id, value, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除该会话全部会话级授权（`s:<conversationId>|` 前缀），全局授权不动（I38h §6.2）。
///
/// 与工作区写入同事务调用；按会话 id 前缀匹配，不按资源路径过滤。
pub fn clear_session_grants(conn: &Connection, conversation_id: &str) -> Result<usize, String> {
    let prefix = format!("s:{conversation_id}|");
    conn.execute(
        "DELETE FROM permission_grants WHERE instr(grant_key, ?1) = 1",
        params![prefix],
    )
    .map_err(|e| e.to_string())
}

/// 读取会话工作区绑定值（空串 = 未配置）。会话不存在返回 `None`。
pub fn get_conversation_workspace(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT workspace_root FROM conversations WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query(params![conversation_id])
        .map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(row.get(0).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}
