use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::conversations::{ConversationRow, MessageRow};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    #[serde(default)]
    pub active_branch_id: Option<String>,
    #[serde(default)]
    pub mode_id: Option<String>,
    /// 会话级工具配置；键恒在，未配置时为 `null`（agent 链路回退到模式初始值）。
    #[serde(default)]
    pub tool_config: Option<super::agent::ConversationToolConfigDto>,
}

impl From<ConversationRow> for ConversationDto {
    fn from(r: ConversationRow) -> Self {
        let tool_config = r
            .tool_config_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok());
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
            mode_id: r.mode_id,
            tool_config,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageToolCallStatusDto {
    Success,
    Error,
    Cancelled,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageToolCallDto {
    pub call_id: String,
    pub name: String,
    #[ts(type = "unknown")]
    pub arguments: serde_json::Value,
    pub status: MessageToolCallStatusDto,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub sibling_index: i64,
    #[serde(default)]
    pub sibling_count: Option<i64>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<MessageToolCallDto>,
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
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDetailDto {
    pub conversation: ConversationDto,
    pub messages: Vec<MessageDto>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationPatchDto {
    #[ts(optional = nullable)]
    pub title: Option<String>,
    #[ts(optional = nullable)]
    pub title_source: Option<String>,
    #[ts(optional = nullable)]
    pub model_id: Option<String>,
    #[ts(optional = nullable)]
    pub provider_id: Option<String>,
    /// 会话当前模式；切换只影响下一次发送，不回溯已有消息（I38d §4.8）。
    #[ts(optional = nullable)]
    pub mode_id: Option<String>,
    /// 会话级工具配置；缺省或 `null` 表示不改。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tool_config: Option<super::agent::ConversationToolConfigDto>,
}

/// 会话详情查询模式（封闭集合）。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum ConversationQueryModeDto {
    #[default]
    Paginated,
    All,
}

impl ConversationQueryModeDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paginated => "paginated",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationQueryOptsDto {
    #[serde(default)]
    pub mode: ConversationQueryModeDto,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[ts(optional = nullable)]
    pub before_timestamp: Option<i64>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendChatMessageArgsDto {
    pub content: String,
    #[ts(optional = nullable)]
    pub mode_id: Option<String>,
    #[ts(optional = nullable)]
    pub branch_point_message_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    #[ts(optional = nullable)]
    pub reasoning_level: Option<String>,
    /// 每轮有效工具配置（agent 链路 = 会话持久值 ?? 模式初始值；chat 链路恒 `null`）。
    /// 缺省即 deny-by-default：模型看不到任何工具。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tool_config: Option<super::agent::ConversationToolConfigDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageDto {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: Option<u32>,
    #[serde(default)]
    pub cache_write_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning_tokens: Option<u32>,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageOccupancyDto {
    /// `None` = 该分支无记录，UI 显示「—」
    #[serde(default)]
    pub used_tokens: Option<i64>,
    #[serde(default)]
    pub completion_tokens: Option<i64>,
    #[serde(default)]
    pub cache_read_tokens: Option<i64>,
    #[serde(default)]
    pub cache_write_tokens: Option<i64>,
    #[serde(default)]
    pub reasoning_tokens: Option<i64>,
    /// 窗口未知时必须缺省，不得填 0 或硬编码常量
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
    /// 与绝对数互斥：窗口未知时缺省
    #[serde(default)]
    pub percentage: Option<f32>,
    #[serde(default)]
    pub estimated: bool,
    /// `estimated = true` 时标明来源；`false` 时缺省
    #[serde(default)]
    pub estimated_source: Option<TokenUsageEstimatedSourceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCumulativeDto {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 估算来源：区分「记录表的估算记录」与「分支零记录时的出站投影」
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum TokenUsageEstimatedSourceDto {
    /// provider 未上报输入 token，由 `total - completion` 回退后落库
    Record,
    /// 该分支零记录，按出站投影估算（只读、不落库）
    Projection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCacheDto {
    pub hit_tokens: i64,
    pub denominator_tokens: i64,
    /// 分母为 0 时不可判定，缺省而不是 0
    #[serde(default)]
    pub hit_rate: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageSummaryDto {
    pub occupancy: TokenUsageOccupancyDto,
    /// 该会话零记录时缺省（UI 显示「—」），不得用 0 冒充「没花过」
    #[serde(default)]
    pub cumulative: Option<TokenUsageCumulativeDto>,
    /// 无「具备缓存能力且上报缓存字段」的请求时缺省
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageAttachmentRefDto {
    pub attachment_id: String,
    pub media_type: Option<String>,
    pub bytes: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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
