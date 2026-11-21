use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 启动配置输入（创建与整体替换共用）。`cwd` / `callTimeoutMs` 缺省表示不设置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpLaunchInputDto {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub call_timeout_ms: Option<u64>,
}

/// 新建：`id` 由后端派生（展示名规范化），因此输入不携带。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerCreateDto {
    pub name: String,
    pub launch: McpLaunchInputDto,
}

/// 更新：`id` 只读；`launch` 缺省（或 `null`）表示只改展示名，不改启动配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerUpdateDto {
    pub id: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub launch: Option<McpLaunchInputDto>,
}

/// 运行时状态投影（I38e §4.1）；只有 `stopped` / `ready` 是稳定态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum McpServerStatusDto {
    Stopped,
    Starting,
    Ready,
    Error,
}

/// 列表 / 详情行：启动配置可见，但 `env` 只回键名，值一律掩码（I38e §6.4）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDto {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub status: McpServerStatusDto,
    pub last_error: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// `env` 的键（有序）；值不回显，编辑需重新填写。
    pub env_keys: Vec<String>,
    pub call_timeout_ms: Option<u64>,
    /// 密文无法解密或明文非法时的原因；非空时该 server 不可启动。
    pub config_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
