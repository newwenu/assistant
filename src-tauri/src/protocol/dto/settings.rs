use assistant_storage::agent_modes::AgentModeRow;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 模式类型（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum AgentModeKindDto {
    Default,
    Quick,
    Deep,
    Custom,
}

impl AgentModeKindDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Quick => "quick",
            Self::Deep => "deep",
            Self::Custom => "custom",
        }
    }
}

impl From<&str> for AgentModeKindDto {
    fn from(value: &str) -> Self {
        match value {
            "default" => Self::Default,
            "quick" => Self::Quick,
            "deep" => Self::Deep,
            _ => Self::Custom,
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
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AgentModeDto {
    pub fn from_row(row: &AgentModeRow) -> Self {
        let tools: Vec<String> = serde_json::from_str(&row.tools_json).unwrap_or_default();
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
            tools,
            presentation_rank: row.presentation_rank,
            is_builtin: row.is_builtin,
            created_at: row.created_at,
            updated_at: row.updated_at,
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
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
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
