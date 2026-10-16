use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentModeWire {
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
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentModeInputWire {
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
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelConfigWire {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ThemeChangedPayload {
    pub resolved: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderInputWire {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub api_key_changed: bool,
    pub models: String,
    pub default_model: String,
}