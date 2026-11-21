// 域类型 → DTO 桥接（I62 §4.4）：按 Rust 孤儿规则归属本 crate。

use crate::base::ProviderErrorPayload;
use crate::base::{ProviderError, ProviderErrorCode};
use crate::types::chat_delta::RetryNotification;
use crate::types::chat_delta::TokenUsage;
use assistant_protocol::*;

impl From<crate::types::ImageAttachmentRef> for ImageAttachmentRefDto {
    fn from(r: crate::types::ImageAttachmentRef) -> Self {
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

impl From<crate::types::FileAttachmentRef> for FileAttachmentRefDto {
    fn from(r: crate::types::FileAttachmentRef) -> Self {
        Self {
            attachment_id: r.attachment_id,
            name: r.name,
            bytes: r.bytes,
        }
    }
}

impl From<crate::types::ContentBlock> for ContentBlockDto {
    fn from(b: crate::types::ContentBlock) -> Self {
        match b {
            crate::types::ContentBlock::Text { text } => Self::Text { text },
            crate::types::ContentBlock::Image { attachment, .. } => Self::Image {
                attachment: ImageAttachmentRefDto::from(attachment),
            },
            crate::types::ContentBlock::File { attachment, .. } => Self::File {
                attachment: FileAttachmentRefDto::from(attachment),
            },
            crate::types::ContentBlock::Thinking {
                thinking,
                signature,
            } => Self::Thinking {
                thinking,
                signature,
            },
            crate::types::ContentBlock::ToolUse { id, name, input } => {
                Self::ToolUse { id, name, input }
            }
            crate::types::ContentBlock::ToolResult { id, output } => {
                Self::ToolResult { id, output }
            }
        }
    }
}

impl From<RetryNotification> for RetryNotificationDto {
    fn from(r: RetryNotification) -> Self {
        Self {
            attempt: r.attempt,
            max_attempts: r.max_attempts,
            delay_ms: r.delay_ms,
            message: r.message,
            error_code: r.error_code.map(CommandErrorCodeDto::from),
        }
    }
}

impl From<ProviderErrorPayload> for ProviderErrorPayloadDto {
    fn from(p: ProviderErrorPayload) -> Self {
        Self {
            code: CommandErrorCodeDto::from(p.code),
            message: p.message,
            retry_after_ms: p.retry_after_ms,
            retryable: p.retryable,
            http_status: p.http_status,
            provider: p.provider,
            provider_code: p.provider_code,
        }
    }
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

/// 穷尽 `match`：`assistant-providers` 新增码时此处编译失败，不会静默漏映射。
impl From<ProviderErrorCode> for CommandErrorCodeDto {
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

impl From<ProviderError> for CommandErrorDto {
    fn from(e: ProviderError) -> Self {
        let code = CommandErrorCodeDto::from(e.code());
        Self {
            code,
            message: e.to_string(),
            retryable: code.is_retryable(),
            retry_after_ms: e.retry_after_ms(),
        }
    }
}
