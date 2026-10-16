use assistant_providers::base::ProviderErrorPayload;
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ChatDeltaPayload {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    pub delta_text: Option<String>,
    pub delta_reasoning: Option<String>,
    pub delta_tool_call_id: Option<String>,
    pub delta_tool_call_name: Option<String>,
    pub delta_tool_call_arguments: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_retry: Option<RetryNotification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProviderErrorPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatStreamStartPayload {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationTerminatedPayload {
    pub conversation_id: String,
    pub summary: String,
    pub is_full: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationCompactFailedPayload {
    pub conversation_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationUnlockedPayload {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatRegeneratePayload {
    pub conversation_id: String,
    pub deleted_message_id: String,
}