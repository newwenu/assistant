use assistant_storage::agent_modes::AgentModeRow;
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

impl AgentModeDto {
    pub fn from_row(row: &AgentModeRow) -> Self {
        // 旧三列（`tools` / `max_tool_tier` / `approval_level`）已手动删除，代码侧不再创建、不再读取：
        // `tool_config_json` 缺失时按契约缺省兜底；内置行的模式初始值由 storage 种子落库。
        let tool_config = row
            .tool_config_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            description: row.description.clone(),
            kind: AgentModeKindDto::from(row.kind.as_str()),
            prompt: row.prompt.clone(),
            model: row.model.clone(),
            temperature: row.temperature,
            max_tokens: row.max_tokens,
            reasoning_enabled: row.reasoning_enabled,
            reasoning_effort: row.reasoning_effort.clone(),
            presentation_rank: row.presentation_rank,
            is_builtin: row.is_builtin,
            created_at: row.created_at,
            updated_at: row.updated_at,
            tool_config,
            avatar: row.avatar.clone(),
        }
    }
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

/// 目录声明的推理可选项投影；`None`（外层 `Option`）表示目录没有可用信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReasoningLevelsDto {
    /// 模型提供开启/关闭开关
    #[serde(default)]
    pub toggle: bool,
    /// 可选的 effort 档位，保持目录顺序
    #[serde(default)]
    pub efforts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigDto {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub support_vision: bool,
    /// 输入模态列表（"text"/"image"/"pdf"/"audio"/"video"）；
    /// 存量 models_json 无此字段，default 空数组（回退 supportVision）。
    #[serde(default)]
    pub input_modalities: Vec<String>,
    /// 目录声明的档位；存量 models_json 无此字段，缺省即 None（走未知模型策略）。
    #[serde(default)]
    pub reasoning_levels: Option<ReasoningLevelsDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInputDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub api_key_changed: Option<bool>,
    pub models: Vec<ModelConfigDto>,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderListDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key_masked: String,
    pub models: Vec<ModelConfigDto>,
    pub default_model: String,
    pub updated_at: i64,
}

/// provider 变更事件的动作（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
pub enum SettingProviderChangedActionDto {
    ProviderSaved,
    ProviderDeleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingProviderChangedPayloadDto {
    pub action: SettingProviderChangedActionDto,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutWarningPayloadDto {
    pub id: String,
    pub binding: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeChangedPayloadDto {
    pub resolved: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallMascotAssetResultDto {
    pub installed_path: String,
    pub bytes: u64,
    pub version: u64,
}