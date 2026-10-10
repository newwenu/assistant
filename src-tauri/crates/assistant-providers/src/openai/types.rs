use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
pub(crate) struct ChatRequest {
    pub model: String,
    pub messages: Vec<Value>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
}

#[derive(Serialize)]
pub(crate) struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatResponseChunk {
    #[serde(default)]
    #[allow(dead_code)]
    pub object: Option<String>,
    pub choices: Option<Vec<ChatChoice>>,
    pub usage: Option<ChatUsage>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatChoice {
    pub delta: Option<ChatDeltaContent>,
    #[serde(default)]
    pub message: Option<ChatDeltaContent>,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatDeltaContent {
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub reasoning_details: Option<Vec<ReasoningDetail>>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReasoningDetail {
    #[serde(rename = "type", default)]
    pub detail_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatToolCallDelta {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<ChatToolCallFunctionDelta>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatToolCallFunctionDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    #[serde(default)]
    pub prompt_tokens_details: Option<ChatPromptTokensDetails>,
    #[serde(default)]
    pub prompt_cache_hit_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ModelsResponse {
    pub data: Option<Vec<ModelItem>>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ModelItem {
    pub id: String,
    #[serde(default)]
    pub owned_by: Option<String>,
}
