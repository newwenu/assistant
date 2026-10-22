use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::{
    agent_modes::TokenUsageSummary,
    conversations::{ConversationRow, MessageRow},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDto {
    pub id: String,
    pub title: String,
    pub title_source: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub generation: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_full: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_branch_id: Option<String>,
}

impl From<ConversationRow> for ConversationDto {
    fn from(r: ConversationRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            title_source: r.title_source,
            provider_id: r.provider_id,
            model_id: r.model_id,
            system_prompt: r.system_prompt,
            is_pinned: r.is_pinned,
            sort_order: r.sort_order,
            generation: r.generation,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_full: r.is_full,
            active_branch_id: r.active_branch_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<i64>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls_json: Option<String>,
    #[serde(default)]
    pub sibling_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sibling_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

impl From<MessageRow> for MessageDto {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            tokens: r.tokens,
            timestamp: r.timestamp,
            parent_id: r.parent_id,
            is_pinned: r.is_pinned,
            reasoning: r.reasoning,
            tool_calls_json: r.tool_calls_json,
            sibling_index: r.sibling_index,
            sibling_count: None,
            provider_id: r.provider_id,
            model_id: r.model_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDetailDto {
    pub conversation: ConversationDto,
    pub messages: Vec<MessageDto>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageDto {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
    #[serde(default)]
    pub context_window_tokens: u32,
}

impl From<TokenUsage> for TokenUsageDto {
    fn from(u: TokenUsage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_write_tokens: u.cache_write_tokens,
            context_window_tokens: u.context_window_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageSummaryDto {
    pub total_tokens: i64,
    pub context_window_tokens: u32,
    pub percentage: f32,
}

impl From<TokenUsageSummary> for TokenUsageSummaryDto {
    fn from(s: TokenUsageSummary) -> Self {
        Self {
            total_tokens: s.total_tokens,
            context_window_tokens: s.context_window_tokens,
            percentage: s.percentage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetryNotificationDto {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ProviderErrorCode>,
}

impl From<RetryNotification> for RetryNotificationDto {
    fn from(r: RetryNotification) -> Self {
        Self {
            attempt: r.attempt,
            max_attempts: r.max_attempts,
            delay_ms: r.delay_ms,
            message: r.message,
            error_code: r.error_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderErrorPayloadDto {
    pub code: ProviderErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
}

impl From<ProviderErrorPayload> for ProviderErrorPayloadDto {
    fn from(p: ProviderErrorPayload) -> Self {
        Self {
            code: p.code,
            message: p.message,
            retry_after_ms: p.retry_after_ms,
            retryable: p.retryable,
            http_status: p.http_status,
            provider: p.provider,
            provider_code: p.provider_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatDeltaPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsageDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_retry: Option<RetryNotificationDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProviderErrorPayloadDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatStreamStartPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationTerminatedPayloadDto {
    pub conversation_id: String,
    pub summary: String,
    pub is_full: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationCompactFailedPayloadDto {
    pub conversation_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationUnlockedPayloadDto {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationListChangedPayloadDto {
    pub change_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatRegeneratePayloadDto {
    pub conversation_id: String,
    pub deleted_message_id: String,
}
