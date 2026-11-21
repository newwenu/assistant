use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent::ConversationToolConfigDto;

/// 模式类型（封闭集合）。只有 `Agent` 走 agent 链路，其余一律 chat。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum AgentModeKindDto {
    Chat,
    Agent,
}

impl AgentModeKindDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Agent => "agent",
        }
    }
}

impl From<&str> for AgentModeKindDto {
    /// 旧库值（`default` / `quick` / `deep` / `custom`）退化为兼容标签，一律读作 `chat`。
    fn from(value: &str) -> Self {
        match value {
            "agent" => Self::Agent,
            _ => Self::Chat,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentModeDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: AgentModeKindDto,
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
    /// 模式级工具装配；键恒在，`tool_config_json` 缺失时取契约缺省。
    pub tool_config: ConversationToolConfigDto,
    /// 头像标识（I43 §3.5）：键恒在，`null` = 未配置，渲染侧走降级链。
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentModeInputDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: AgentModeKindDto,
    pub prompt: String,
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub temperature: Option<f32>,
    #[ts(optional = nullable)]
    pub max_tokens: Option<u32>,
    #[ts(optional = nullable)]
    pub reasoning_enabled: Option<bool>,
    #[ts(optional = nullable)]
    pub reasoning_effort: Option<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    /// 模式级工具装配；缺省或 `null` 表示沿用库内现值。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tool_config: Option<ConversationToolConfigDto>,
    /// 头像标识（I43 §3.5 三态）：缺省 / `null` = 沿用库内现值；空串 = 清除为
    /// `NULL`；其余原样落库（合法性由运行时兜底，不在本层校验）。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub avatar: Option<String>,
}
