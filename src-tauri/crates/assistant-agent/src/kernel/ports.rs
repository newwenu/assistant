use std::sync::Arc;

use assistant_providers::base::{BaseProviderAdapter, ChatDelta, ProviderErrorPayload};
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::conversations::ConversationDetail;
use assistant_storage::providers::ProviderRow;
use assistant_storage::token_usage::TokenUsageRecord;
use async_trait::async_trait;
use serde::Serialize;

use crate::chat_options::ReasoningOverride;

use super::outcome::TurnStatus;
use super::phase::TurnPhase;

/// provider 流片段（provider 中立流类型，非 wire）。
pub type ProviderDelta = ChatDelta;

/// 终态携带的错误信息。复用 provider 已定义的序列化错误载荷，避免内核自造错误类型。
pub type ChatError = ProviderErrorPayload;

/// 运行时事件。适配层负责映射为线事件名，内核不写字符串字面量。
///
/// `Serialize` 只服务测试轨迹比对（`kernel_emit`），不承载 wire 契约：
/// 线事件一律经 `protocol/dto` 投影。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TurnEvent {
    StreamStart {
        turn_id: String,
        phase: TurnPhase,
    },
    Delta {
        turn_id: String,
        seq: u32,
        /// 轮内迭代序号，从 1 起；provider 重试不递增。
        iteration: u32,
        delta: ProviderDelta,
    },
    PhaseChanged {
        turn_id: String,
        phase: TurnPhase,
    },
    ToolStarted {
        turn_id: String,
        call_id: String,
        name: String,
    },
    ToolFinished {
        turn_id: String,
        call_id: String,
        ok: bool,
    },
    CompactionStarted {
        turn_id: String,
    },
    CompactionFinished {
        turn_id: String,
        ok: bool,
    },
    Terminated {
        turn_id: String,
        status: TurnStatus,
        /// 本 turn 最后一条 `Delta` 的 `seq`（无增量时为 0）。终止错误事件沿用该值，
        /// 避免适配层凭空造 `seq`。
        last_seq: u32,
        error: Option<ChatError>,
    },
    UserMessageConfirmed {
        conversation_id: String,
        user_msg_id: String,
    },
}

/// 事件出口。同步且无返回值：发射失败只记日志，不得让 turn 失败（design §1.7）。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: TurnEvent);
}

/// 消息终态，与 `messages.status` 列取值一致（Step 8.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Streaming,
    Completed,
    Interrupted,
    Failed,
}

impl MessageStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Streaming => "streaming",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateAssistantReq {
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum StoreError {
    NotFound(String),
    Failed(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Failed(msg) => write!(f, "store failure: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// 落库用的 usage 记录，复用 storage 已定义的字段结构。
pub type UsageRecord = TokenUsageRecord;

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn create_assistant_message(&self, req: CreateAssistantReq)
        -> Result<String, StoreError>;

    async fn save_content(
        &self,
        msg_id: &str,
        content: &str,
        tokens: Option<u32>,
        reasoning: Option<&str>,
    ) -> Result<(), StoreError>;

    async fn finalize(&self, msg_id: &str, status: MessageStatus) -> Result<(), StoreError>;

    async fn record_usage(&self, records: &[UsageRecord]) -> Result<(), StoreError>;
}

#[derive(Clone)]
pub struct TurnInput {
    pub detail: ConversationDetail,
    /// 本次发送所选的 mode（chat 可为 `None`，与旧链路一致）。
    pub mode: Option<AgentModeRow>,
    pub reasoning: ReasoningOverride,
    /// 会话当前 provider 行 + 已解密 key，由端口装配，`request::build` 不再读 DB。
    pub provider: ProviderCredential,
    /// 附件读盘句柄，同样由端口给出。
    pub attachments: Arc<AttachmentStore>,
    /// provider 适配器由输入端口构造，内核只消费不构造（H1）：
    /// 生产由 `create_provider_adapter` 给出，L1 用例注入脚本化适配器。
    pub adapter: Arc<dyn BaseProviderAdapter>,
}

#[derive(Debug, Clone)]
pub struct ProviderCredential {
    pub row: ProviderRow,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub enum InputError {
    NotFound(String),
    Failed(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Failed(msg) => write!(f, "input failure: {msg}"),
        }
    }
}

impl std::error::Error for InputError {}

#[async_trait]
pub trait TurnInputSource: Send + Sync {
    async fn conversation_detail(&self, conversation_id: &str) -> Result<TurnInput, InputError>;

    /// 只读；标题兜底等写操作留在宿主命令层（design §1.6 第 3 条）。
    async fn provider(&self, provider_id: &str) -> Result<ProviderCredential, InputError>;

    fn attachments(&self) -> Arc<AttachmentStore>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> std::time::Instant;

    /// 墙钟毫秒时间戳（Unix epoch）。落库时间戳一律由此取得，保证单源且可注入。
    fn now_millis(&self) -> i64;
}

/// 终止式压缩的触发端口。内核只做判定，压缩本身由宿主异步执行。
#[async_trait]
pub trait CompactionSink: Send + Sync {
    async fn terminate(&self, conversation_id: &str);
}

#[cfg(any(test, feature = "test-doubles"))]
pub mod test_doubles;
