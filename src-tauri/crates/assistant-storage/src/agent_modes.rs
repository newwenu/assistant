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
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// 模式级工具装配的落库形态（`ModeToolConfig` 的 JSON）。
    /// 内置行的初始值由本模块 `BUILTIN_AGENT_TOOL_CONFIG` / `ModeToolConfig::default()` 播种；
    /// 读到 `NULL`（旧库行）时由协议层按契约缺省兜底。
    pub tool_config_json: Option<String>,
    /// 头像标识（I43 §3.5）：`NULL` = 未配置，`builtin:<name>` = 内置 SVG，
    /// `data:image/*` = 内联图。渲染侧按降级链兜底，本层不做校验。
    pub avatar: Option<String>,
}

const MODE_SELECT: &str = "id, name, description, kind, prompt, model, temperature, max_tokens, reasoning_enabled, reasoning_effort, presentation_rank, is_builtin, created_at, updated_at, tool_config_json, avatar";

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
        presentation_rank: row.get::<_, i64>(10)? as u32,
        is_builtin: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        tool_config_json: row.get(14)?,
        avatar: row.get(15)?,
    })
}

pub fn insert_mode(conn: &Connection, row: &AgentModeRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO agent_modes ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
            row.presentation_rank as i64,
            row.is_builtin as i64,
            row.created_at,
            row.updated_at,
            row.tool_config_json,
            row.avatar
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
        "UPDATE agent_modes SET name=?2, description=?3, kind=?4, prompt=?5, model=?6, temperature=?7, max_tokens=?8, reasoning_enabled=?9, reasoning_effort=?10, presentation_rank=?11, is_builtin=?12, updated_at=?13, tool_config_json=?14, avatar=?15 WHERE id=?1",
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
            row.presentation_rank as i64,
            row.is_builtin as i64,
            row.updated_at,
            row.tool_config_json,
            row.avatar
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

/// 工具层级（`tool_config_json` 的 `maxToolTier` 取值）。
///
/// 存储层自有类型：`assistant-storage` 不依赖 app 的协议层，故在此定义落库形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolTier {
    #[default]
    Sandbox,
    Os,
    External,
}

/// 审批级别（`tool_config_json` 的 `approvalLevel` 取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevel {
    #[default]
    None,
    DangerousOnly,
    AllOsTools,
}

/// 模式级工具装配的落库形态（`agent_modes.tool_config_json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeToolConfig {
    pub max_tool_tier: ToolTier,
    pub approval_level: ApprovalLevel,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
}

impl Default for ModeToolConfig {
    /// 缺省装配：沙箱层级、不额外审批、空白名单（deny-by-default）。
    fn default() -> Self {
        Self {
            max_tool_tier: ToolTier::Sandbox,
            approval_level: ApprovalLevel::None,
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
        }
    }
}

/// `builtin-agent` 的模式级初始装配：沙箱层级 + 仅危险操作审批 + 空白名单。
///
/// 这是内置模式的**业务种子**，与 `ModeToolConfig::default()`（缺省兜底）语义不同。
/// 工具装配初始值只在此处定义：协议层只有契约缺省，app 层不做任何默认值。
pub const BUILTIN_AGENT_TOOL_CONFIG: ModeToolConfig = ModeToolConfig {
    max_tool_tier: ToolTier::Sandbox,
    approval_level: ApprovalLevel::DangerousOnly,
    enabled_tools: Vec::new(),
    disabled_tools: Vec::new(),
};

pub fn encode_tool_config(config: &ModeToolConfig) -> String {
    serde_json::to_string(config).expect("mode tool config is serializable")
}

fn builtin_defaults(now: i64, resolve: &dyn Fn(&str) -> String) -> Vec<AgentModeRow> {
    let default_config = ModeToolConfig::default();
    let default_config_json = Some(encode_tool_config(&default_config));
    let agent_config_json = Some(encode_tool_config(&BUILTIN_AGENT_TOOL_CONFIG));

    vec![
        AgentModeRow {
            id: "builtin-default".to_string(),
            name: resolve("agent_mode.builtin.default.name"),
            description: resolve("agent_mode.builtin.default.description"),
            kind: "chat".to_string(),
            prompt: String::new(),
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            reasoning_enabled: None,
            reasoning_effort: None,
            presentation_rank: 0,
            is_builtin: true,
            created_at: now,
            updated_at: now,
            tool_config_json: default_config_json.clone(),
            avatar: Some("builtin:chat".to_string()),
        },
        AgentModeRow {
            id: "builtin-agent".to_string(),
            name: resolve("agent_mode.builtin.agent.name"),
            description: resolve("agent_mode.builtin.agent.description"),
            kind: "agent".to_string(),
            prompt: String::new(),
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            reasoning_enabled: None,
            reasoning_effort: None,
            presentation_rank: 1,
            is_builtin: true,
            created_at: now,
            updated_at: now,
            tool_config_json: agent_config_json,
            avatar: Some("builtin:agent".to_string()),
        },
        AgentModeRow {
            id: "builtin-translate".to_string(),
            name: resolve("agent_mode.builtin.translate.name"),
            description: resolve("agent_mode.builtin.translate.description"),
            kind: "chat".to_string(),
            prompt: resolve("agent_mode.builtin.translate.prompt"),
            model: None,
            temperature: Some(0.3),
            max_tokens: Some(4096),
            reasoning_enabled: None,
            reasoning_effort: None,
            presentation_rank: 10,
            is_builtin: true,
            created_at: now,
            updated_at: now,
            tool_config_json: default_config_json,
            avatar: Some("builtin:translate".to_string()),
        },
    ]
}

pub fn seed_builtin_modes(
    conn: &Connection,
    resolve: &dyn Fn(&str) -> String,
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    for mode in &builtin_defaults(now, resolve) {
        insert_mode(conn, mode)?;
    }
    Ok(())
}

pub fn reset_builtin_mode(
    conn: &Connection,
    id: &str,
    resolve: &dyn Fn(&str) -> String,
) -> Result<(), StorageError> {
    let now = chrono::Utc::now().timestamp_millis();
    let defaults = builtin_defaults(now, resolve);
    let default_mode = defaults
        .iter()
        .find(|m| m.id == id)
        .ok_or(StorageError::NotBuiltinModeCannotReset)?;

    conn.execute(
        "UPDATE agent_modes SET name=?2, description=?3, prompt=?4, model=?5, temperature=?6, max_tokens=?7, reasoning_enabled=?8, reasoning_effort=?9, updated_at=?10, tool_config_json=?11, avatar=?12 WHERE id=?1",
        params![
            id,
            default_mode.name,
            default_mode.description,
            default_mode.prompt,
            default_mode.model,
            default_mode.temperature,
            default_mode.max_tokens,
            default_mode.reasoning_enabled.map(|b| b as i64),
            default_mode.reasoning_effort,
            now,
            default_mode.tool_config_json,
            default_mode.avatar
        ],
    )
    .map_err(|e| StorageError::Other(e.to_string()))?;
    Ok(())
}