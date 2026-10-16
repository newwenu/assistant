use serde::Serialize;
use crate::protocol::ModelConfigWire;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderListRow {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key_masked: String,
    pub models: Vec<ModelConfigWire>,
    pub default_model: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SettingProviderChangedPayload {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ShortcutWarningPayload {
    pub id: String,
    pub binding: String,
    pub error: String,
}