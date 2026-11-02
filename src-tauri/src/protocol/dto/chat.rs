use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::conversations::{ConversationRow, MessageRow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub enum ProviderErrorCodeDto {
    ApiKeyMissing,
    AuthFailed,
    RateLimited,
    ModelNotFound,
    ContextOverflow,
    NetworkError,
    Timeout,
    ServerError,
    RequestRejected,
    ParseError,
    Cancelled,
    StreamError,
    InternalError,
}

impl From<ProviderErrorCode> for ProviderErrorCodeDto {
    fn from(code: ProviderErrorCode) -> Self {
        match code {
            ProviderErrorCode::ApiKeyMissing => Self::ApiKeyMissing,
            ProviderErrorCode::AuthFailed => Self::AuthFailed,
            ProviderErrorCode::RateLimited => Self::RateLimited,
            ProviderErrorCode::ModelNotFound => Self::ModelNotFound,
            ProviderErrorCode::ContextOverflow => Self::ContextOverflow,
            ProviderErrorCode::NetworkError => Self::NetworkError,
            ProviderErrorCode::Timeout => Self::Timeout,
            ProviderErrorCode::ServerError => Self::ServerError,
            ProviderErrorCode::RequestRejected => Self::RequestRejected,
            ProviderErrorCode::ParseError => Self::ParseError,
            ProviderErrorCode::Cancelled => Self::Cancelled,
            ProviderErrorCode::StreamError => Self::StreamError,
            ProviderErrorCode::InternalError => Self::InternalError,
        }
    }
}

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
pub struct ConversationPatchDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationQueryOptsDto {
    #[serde(default)]
    pub mode: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_timestamp: Option<i64>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageArgsDto {
    pub content: String,
    pub mode_id: Option<String>,
    pub branch_point_message_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageDto {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
}

impl From<TokenUsage> for TokenUsageDto {
    fn from(u: TokenUsage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_write_tokens: u.cache_write_tokens,
            reasoning_tokens: u.reasoning_tokens,
            context_window_tokens: u.context_window_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageOccupancyDto {
    /// `None` = 该分支无记录，UI 显示「—」
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i64>,
    /// 窗口未知时必须缺省，不得填 0 或硬编码常量
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    /// 与绝对数互斥：窗口未知时缺省
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f32>,
    #[serde(default)]
    pub estimated: bool,
    /// `estimated = true` 时标明来源；`false` 时缺省
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_source: Option<TokenUsageEstimatedSourceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCumulativeDto {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 估算来源：区分「记录表的估算记录」与「分支零记录时的出站投影」
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TokenUsageEstimatedSourceDto {
    /// provider 未上报输入 token，由 `total - completion` 回退后落库
    Record,
    /// 该分支零记录，按出站投影估算（只读、不落库）
    Projection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCacheDto {
    pub hit_tokens: i64,
    pub denominator_tokens: i64,
    /// 分母为 0 时不可判定，缺省而不是 0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_rate: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageSummaryDto {
    pub occupancy: TokenUsageOccupancyDto,
    /// 该会话零记录时缺省（UI 显示「—」），不得用 0 冒充「没花过」
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative: Option<TokenUsageCumulativeDto>,
    /// 无「具备缓存能力且上报缓存字段」的请求时缺省
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<TokenUsageCacheDto>,
}

impl From<assistant_storage::token_usage::OccupancySummary> for TokenUsageOccupancyDto {
    fn from(s: assistant_storage::token_usage::OccupancySummary) -> Self {
        Self {
            used_tokens: s.used_tokens,
            completion_tokens: s.completion_tokens,
            cache_read_tokens: s.cache_read_tokens,
            cache_write_tokens: s.cache_write_tokens,
            reasoning_tokens: s.reasoning_tokens,
            context_window_tokens: s.context_window_tokens,
            percentage: s.percentage,
            estimated: s.estimated,
            estimated_source: s.estimated.then_some(TokenUsageEstimatedSourceDto::Record),
        }
    }
}

impl From<assistant_storage::token_usage::CumulativeSummary> for TokenUsageCumulativeDto {
    fn from(s: assistant_storage::token_usage::CumulativeSummary) -> Self {
        Self {
            total_tokens: s.total_tokens,
            prompt_tokens: s.prompt_tokens,
            completion_tokens: s.completion_tokens,
        }
    }
}

impl From<assistant_storage::token_usage::CacheStats> for TokenUsageCacheDto {
    fn from(s: assistant_storage::token_usage::CacheStats) -> Self {
        Self {
            hit_tokens: s.hit_tokens,
            denominator_tokens: s.denominator_tokens,
            hit_rate: s.hit_rate(),
        }
    }
}

impl From<assistant_storage::token_usage::TokenUsageSummary> for TokenUsageSummaryDto {
    fn from(s: assistant_storage::token_usage::TokenUsageSummary) -> Self {
        Self {
            occupancy: s.occupancy.into(),
            cumulative: s.cumulative.map(Into::into),
            cache: s.cache.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryNotificationDto {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ProviderErrorCodeDto>,
}

impl From<RetryNotification> for RetryNotificationDto {
    fn from(r: RetryNotification) -> Self {
        Self {
            attempt: r.attempt,
            max_attempts: r.max_attempts,
            delay_ms: r.delay_ms,
            message: r.message,
            error_code: r.error_code.map(ProviderErrorCodeDto::from),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderErrorPayloadDto {
    pub code: ProviderErrorCodeDto,
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
            code: ProviderErrorCodeDto::from(p.code),
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
pub struct ChatUserMessageConfirmedPayloadDto {
    pub conversation_id: String,
    pub user_message_id: String,
    pub user_message_content: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageAttachmentRefDto {
    pub attachment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl From<assistant_providers::types::ImageAttachmentRef> for ImageAttachmentRefDto {
    fn from(r: assistant_providers::types::ImageAttachmentRef) -> Self {
        Self {
            attachment_id: r.attachment_id,
            media_type: r.media_type,
            bytes: r.bytes,
            width: r.width,
            height: r.height,
            name: r.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileAttachmentRefDto {
    pub attachment_id: String,
    pub name: String,
    pub bytes: u64,
}

impl From<assistant_providers::types::FileAttachmentRef> for FileAttachmentRefDto {
    fn from(r: assistant_providers::types::FileAttachmentRef) -> Self {
        Self {
            attachment_id: r.attachment_id,
            name: r.name,
            bytes: r.bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDto {
    Text {
        text: String,
    },
    Image {
        attachment: ImageAttachmentRefDto,
    },
    File {
        attachment: FileAttachmentRefDto,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    ToolUse {
        #[serde(rename = "tool_use_id")]
        id: String,
        name: String,
        input: String,
    },
    ToolResult {
        #[serde(rename = "tool_use_id")]
        id: String,
        output: String,
    },
}

impl From<assistant_providers::types::ContentBlock> for ContentBlockDto {
    fn from(b: assistant_providers::types::ContentBlock) -> Self {
        match b {
            assistant_providers::types::ContentBlock::Text { text } => Self::Text { text },
            assistant_providers::types::ContentBlock::Image { attachment, .. } => Self::Image {
                attachment: ImageAttachmentRefDto::from(attachment),
            },
            assistant_providers::types::ContentBlock::File { attachment, .. } => Self::File {
                attachment: FileAttachmentRefDto::from(attachment),
            },
            assistant_providers::types::ContentBlock::Thinking {
                thinking,
                signature,
            } => Self::Thinking {
                thinking,
                signature,
            },
            assistant_providers::types::ContentBlock::ToolUse { id, name, input } => {
                Self::ToolUse { id, name, input }
            }
            assistant_providers::types::ContentBlock::ToolResult { id, output } => {
                Self::ToolResult { id, output }
            }
        }
    }
}
