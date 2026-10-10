use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningContentKind {
    Thinking,
    Reasoning,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallCompletion {
    NormalNoToolUse,
    NormalToolUse,
    OutputLimit,
    ContentFiltered,
    Failed,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_index: Option<usize>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub arguments_is_snapshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTokenUsage {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cache_creation_token_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponseReplayCapture {
    pub protocol: String,
    pub items: Vec<ModelResponseReplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelResponseReplayItem {
    OpaqueReasoning {
        #[serde(skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(default)]
        summary: Vec<ReasoningSummaryPart>,
        opaque_state: String,
    },
    AssistantMessage,
    FunctionCall {
        call_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummaryPart {
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct UnifiedResponse {
    pub text: Option<String>,
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content_kind: Option<ReasoningContentKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    pub tool_call: Option<UnifiedToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_completion: Option<ToolCallCompletion>,
    pub finish_reason: Option<String>,
    pub usage: Option<UnifiedTokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_response_replay: Option<ModelResponseReplayCapture>,
}

impl std::fmt::Debug for UnifiedResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedResponse")
            .field("text", &self.text)
            .field(
                "reasoning_content",
                &self.reasoning_content.as_ref().map(|s| {
                    if s.len() > 100 {
                        format!(
                            "{}...",
                            &s[..s
                                .char_indices()
                                .take(100)
                                .last()
                                .map(|(i, c)| i + c.len_utf8())
                                .unwrap_or(100)
                                .min(s.len())]
                        )
                    } else {
                        s.clone()
                    }
                }),
            )
            .field("reasoning_content_kind", &self.reasoning_content_kind)
            .field("tool_call", &self.tool_call)
            .field("usage", &self.usage)
            .field("tool_call_completion", &self.tool_call_completion)
            .field("finish_reason", &self.finish_reason)
            .finish()
    }
}
