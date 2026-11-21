use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// 能力事实的三态（封闭集合）。`unknown` 与 `unsupported` 语义不同：
/// 前者是「不知道」，后者是「确定不支持」，消费点策略按特性区分。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum CapabilitySupportDto {
    Supported,
    Unsupported,
    Unknown,
}

/// 能力事实的来源（封闭集合），与 `models_json` 里的 `source` 取值一致。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum CapabilitySourceDto {
    Api,
    Catalog,
    Snapshot,
    Fallback,
}

/// 一条能力事实：三态 + 来源。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCapabilityDto {
    pub support: CapabilitySupportDto,
    pub source: CapabilitySourceDto,
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
    /// 视觉能力事实；存量 models_json 无此字段，缺省即 None（由旧字段派生）。
    #[serde(default)]
    pub vision: Option<ModelCapabilityDto>,
    /// 原生 PDF 能力事实；存量 models_json 无此字段，缺省即 None。
    #[serde(default)]
    pub pdf: Option<ModelCapabilityDto>,
    /// 附件能力事实；存量 models_json 无此字段，缺省即 None。
    #[serde(default)]
    pub attachment: Option<ModelCapabilityDto>,
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

/// 单模型的档位可表达投影。**只读输出**：不进 `ModelConfigDto`，
/// 避免被写回 `models_json` 成过期快照。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelEffortSupportDto {
    pub model_id: String,
    /// 该端点可原样下发的档位；端点不归一（responses）时等于目录声明档位。
    pub expressible: Vec<String>,
    /// 目录声明但会被该端点丢弃的档位（保序）；别名档位不计入。
    pub unsupported: Vec<String>,
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
    /// 档位可表达投影（只读），按 `models` 顺序一一对应。
    #[serde(default)]
    pub effort_support: Vec<ModelEffortSupportDto>,
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
