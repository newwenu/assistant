use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct MessagesRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct MessageStart {
    pub message: MessageStartMessage,
}

#[derive(Deserialize, Debug)]
pub(crate) struct MessageStartMessage {
    pub usage: Option<AnthropicUsage>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub(crate) struct AnthropicUsage {
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
}

impl AnthropicUsage {
    pub fn update(&mut self, other: &AnthropicUsage) {
        if other.input_tokens.is_some() {
            self.input_tokens = other.input_tokens;
        }
        if other.output_tokens.is_some() {
            self.output_tokens = other.output_tokens;
        }
        if other.cache_read_input_tokens.is_some() {
            self.cache_read_input_tokens = other.cache_read_input_tokens;
        }
        if other.cache_creation_input_tokens.is_some() {
            self.cache_creation_input_tokens = other.cache_creation_input_tokens;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_input_tokens.is_none()
            && self.cache_creation_input_tokens.is_none()
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct MessageDelta {
    pub delta: MessageDeltaDelta,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct MessageDeltaDelta {
    pub stop_reason: Option<String>,
    #[allow(dead_code)]
    pub stop_sequence: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ContentBlockStart {
    pub index: Option<usize>,
    pub content_block: ContentBlock,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking {
        thinking: Option<String>,
        signature: Option<String>,
    },
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ContentBlockDelta {
    pub index: Option<usize>,
    pub delta: ContentBlockDeltaType,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum ContentBlockDeltaType {
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
    #[serde(rename = "signature_delta")]
    Signature { signature: String },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub(crate) struct AnthropicSSEError {
    pub error: AnthropicSSEErrorDetails,
}

#[derive(Deserialize, Debug)]
pub(crate) struct AnthropicSSEErrorDetails {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct AnthropicModelsResponse {
    pub data: Vec<AnthropicModelEntry>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct AnthropicModelEntry {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub max_input_tokens: Option<u32>,
    #[serde(default)]
    pub limit: Option<AnthropicModelLimit>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct AnthropicModelLimit {
    #[serde(default)]
    pub context: Option<u32>,
}

impl AnthropicModelEntry {
    pub(crate) fn resolved_context_window(&self) -> Option<u32> {
        [
            self.context_window,
            self.max_input_tokens,
            self.limit.as_ref().and_then(|l| l.context),
        ]
        .into_iter()
        .flatten()
        .find(|&v| v > 0)
    }
}
