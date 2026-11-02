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
    pub completion_tokens_details: Option<ChatCompletionTokensDetails>,
    #[serde(default)]
    pub prompt_cache_hit_tokens: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub prompt_cache_miss_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChatCompletionTokensDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ModelsResponse {
    #[serde(default)]
    pub data: Option<Vec<ModelItem>>,
    #[serde(default)]
    pub models: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct ModelArchitecture {
    #[serde(default)]
    pub modality: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ModelItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub owned_by: Option<String>,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_input_tokens: Option<u32>,
    #[serde(default)]
    pub limit: Option<ModelLimit>,
    #[serde(default)]
    pub top_provider: Option<TopProvider>,
    #[serde(default)]
    pub architecture: Option<ModelArchitecture>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct ModelLimit {
    #[serde(default)]
    pub context: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct TopProvider {
    #[serde(default)]
    pub context_length: Option<u32>,
}

impl ModelItem {
    pub(crate) fn resolved_context_window(&self) -> Option<u32> {
        [
            self.context_window,
            self.context_length,
            self.max_input_tokens,
            self.limit.as_ref().and_then(|l| l.context),
            self.top_provider.as_ref().and_then(|t| t.context_length),
        ]
        .into_iter()
        .flatten()
        .find(|&v| v > 0)
    }

    pub(crate) fn resolved_id(&self) -> Option<String> {
        if !self.id.as_ref().is_some_and(|s| s.is_empty()) {
            self.id.clone()
        } else {
            None
        }
    }

    pub(crate) fn resolved_name(&self) -> String {
        self.name
            .as_deref()
            .or(self.display_name.as_deref())
            .unwrap_or("")
            .to_string()
    }

    pub(crate) fn resolved_support_vision(&self) -> Option<bool> {
        self.architecture.as_ref().and_then(|arch| {
            arch.modality
                .as_ref()
                .map(|m| m.to_ascii_lowercase().contains("image"))
        })
    }
}
