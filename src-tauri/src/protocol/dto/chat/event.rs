use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::RetryNotification;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::usage::TokenUsageDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryNotificationDto {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderErrorPayloadDto {
    pub code: ProviderErrorCodeDto,
    pub message: String,
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatDeltaPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    /// 轮内迭代序号，从 1 起；provider 重试不递增。chat 链路恒为 1。
    #[serde(default)]
    pub iteration: u32,
    pub delta_text: Option<String>,
    pub delta_reasoning: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsageDto>,
    pub delta_retry: Option<RetryNotificationDto>,
    pub error: Option<ProviderErrorPayloadDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatStreamStartPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    /// 起始 phase；缺省为 `starting`，与内核 `TurnEvent::StreamStart` 同源。
    #[serde(default)]
    pub phase: TurnPhaseDto,
    /// 本轮 turn 序号（I46 §2.6）：键恒在、值 `null` = 无坐标（存量分支点）。
    pub turn_index: Option<u32>,
    /// 首个 assistant 行的迭代号，恒 `1`。
    pub iteration: Option<u32>,
}

/// 与内核 `TurnPhase` 同源：`From` 用穷尽 `match`，内核新增 phase 时此处编译失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum TurnPhaseDto {
    #[default]
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    AwaitingApproval,
    Finalizing,
}

impl From<assistant_agent::kernel::TurnPhase> for TurnPhaseDto {
    fn from(phase: assistant_agent::kernel::TurnPhase) -> Self {
        use assistant_agent::kernel::TurnPhase;
        match phase {
            TurnPhase::Starting => Self::Starting,
            TurnPhase::Compacting => Self::Compacting,
            TurnPhase::Thinking => Self::Thinking,
            TurnPhase::Streaming => Self::Streaming,
            TurnPhase::ToolCalling => Self::ToolCalling,
            TurnPhase::AwaitingApproval => Self::AwaitingApproval,
            TurnPhase::Finalizing => Self::Finalizing,
        }
    }
}

/// 与内核 `SessionState` 同源（封闭集合）；同样用穷尽 `match` 保证不漏变体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SessionStateDto {
    Idle,
    Processing {
        turn_id: String,
        phase: TurnPhaseDto,
    },
    Error {
        message: String,
        recoverable: bool,
    },
}

impl From<assistant_agent::kernel::SessionState> for SessionStateDto {
    fn from(state: assistant_agent::kernel::SessionState) -> Self {
        use assistant_agent::kernel::SessionState;
        match state {
            SessionState::Idle => Self::Idle,
            SessionState::Processing { turn_id, phase } => Self::Processing {
                turn_id,
                phase: phase.into(),
            },
            SessionState::Error {
                message,
                recoverable,
            } => Self::Error {
                message,
                recoverable,
            },
        }
    }
}

/// 状态快照载荷：不带 `seq`（它不是流片段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionStateChangedPayloadDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub state: SessionStateDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatUserMessageConfirmedPayloadDto {
    pub conversation_id: String,
    pub user_message_id: String,
    pub user_message_content: String,
    /// 本轮 turn 序号（I46 §2.6）：键恒在、值 `null` = 无坐标。
    pub turn_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationTerminatedPayloadDto {
    pub conversation_id: String,
    pub is_full: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationUnlockedPayloadDto {
    pub conversation_id: String,
}

/// `conversation-list-changed` 事件的变更原因（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum ConversationListChangeKindDto {
    Renamed,
    Deleted,
    PinToggled,
    Duplicated,
    ModelUpdated,
    ProviderUpdated,
    ModeUpdated,
    ToolConfigUpdated,
    BatchDeleted,
    Purged,
    Created,
    StreamFinished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationListChangedPayloadDto {
    pub change_kind: ConversationListChangeKindDto,
    pub conversation_id: Option<String>,
    pub affected_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatRegeneratePayloadDto {
    pub conversation_id: String,
    pub deleted_message_id: String,
}
