use std::sync::Arc;
use std::time::Duration;

use assistant_providers::base::{BaseProviderAdapter, ChatDelta, ProviderErrorPayload};
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::conversations::ConversationDetail;
use assistant_storage::providers::ProviderRow;
use assistant_storage::token_usage::TokenUsageRecord;
use async_trait::async_trait;
use serde::Serialize;

use crate::agent::node_event::AgentNodeEvent;
use crate::agent::tool_event::ToolEvent;
use crate::chat_options::ReasoningOverride;
use crate::prompt_builder::PromptFacts;

use super::outcome::TurnStatus;
use super::phase::TurnPhase;

/// provider 流片段（provider 中立流类型，非 wire）。
pub type ProviderDelta = ChatDelta;

/// 终态携带的错误信息。复用 provider 已定义的序列化错误载荷，避免内核自造错误类型。
pub type ChatError = ProviderErrorPayload;

/// 运行时事件。适配层负责映射为线事件名，内核不写字符串字面量。
///
/// `Serialize` 只服务测试轨迹比对（`kernel_emit`），不承载 wire 契约：
/// 线事件一律经 `assistant-protocol` 的 DTO 投影。
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
    Tool(ToolEvent),
    /// 节点级观测（I55 §4.2）：与 `Delta` 共用同一 `seq` 序列，`node` / `state` 双判别。
    Node(AgentNodeEvent),
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

/// 代际校验端口：落库前的「本 turn 是否仍是该对话的当前代际」判定。
///
/// 生产实现由 `TurnRunner` 持有 `TurnTasks` 的守卫，内核不感知 `TurnTasks`。
/// 旧代际一律不得落库 / 发事件（§5.1）。
pub trait TurnGeneration: Send + Sync {
    fn is_current(&self) -> bool;
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
    /// turn 起点的模式快照（I43 §3.4）：由 `TurnInput.mode` 取值，内核不读 DB。
    pub mode_id: Option<String>,
    /// 节点坐标（I46 §2.5）：由宿主透传，内核不认识持久坐标、只搬运。
    /// `None` 时该列写 `NULL`（存量会话 / 无坐标分支点），渲染侧落 `unresolved`。
    pub turn_index: Option<u32>,
    /// turn 内迭代序号；`0` = 非迭代行。
    pub iteration: u32,
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

#[derive(Debug, Clone)]
pub struct DeclareToolCallsReq {
    pub conversation_id: String,
    /// 声明这批调用的 assistant 行 id，整批占位行挂在它之下。
    pub parent_id: String,
    pub call_ids: Vec<String>,
    /// 节点坐标（I46 §2.5）：占位行与声明行同属一个 turn 与迭代。
    pub turn_index: Option<u32>,
    pub iteration: u32,
}

#[derive(Debug, Clone)]
pub struct AppendToolMessageReq {
    pub conversation_id: String,
    pub parent_id: String,
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
    pub timestamp: i64,
    /// 节点坐标（I46 §2.5）：tool 行属于发起该调用的 turn 与迭代，不得写 `0`。
    pub turn_index: Option<u32>,
    pub iteration: u32,
}

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

    async fn save_tool_calls(&self, msg_id: &str, tool_calls_json: &str) -> Result<(), StoreError>;

    /// 声明工具调用（D27）：`tool_calls` 落库之后调用，为每个调用插入一条空的占位结果行。
    ///
    /// 返回按序生成的占位行 id（= 整批的新挂载点）。拓扑在此刻定死，结果到达时只回填
    /// 正文——这是「中断后可自愈」的前提：行已存在，投影有落脚点。
    async fn declare_tool_calls(&self, req: DeclareToolCallsReq)
        -> Result<Vec<String>, StoreError>;

    /// 标记「已进入执行队列」（D27）：调用执行端口之前调用，占位行 `pending` → `running`。
    ///
    /// 单向推进、不回退。写失败不阻断执行——状态退化为 `pending` 只会让文案更保守，
    /// 不该让一次可用的工具调用因此失败。
    async fn mark_tool_calls_running(
        &self,
        conversation_id: &str,
        call_ids: &[String],
    ) -> Result<(), StoreError>;

    /// 只回填已存在的占位行，不新建行（D27）：用于 turn 已失效（`!is_current`）的提前返回。
    ///
    /// 走 `append_tool_messages` 会插入新行、改写挂载点；此处调用时已离开当前分支的
    /// 写入窗口，只能原地补正文。没有占位行时静默跳过（存量会话）。
    async fn fill_tool_messages(&self, reqs: Vec<AppendToolMessageReq>) -> Result<(), StoreError> {
        let _ = reqs;
        Ok(())
    }

    /// 一批 tool 结果一次性落库，返回按序的占位行 id。
    ///
    /// 同一批必须原子写入：逐条提交时进程若在批中途退出，就会留下「有调用、无结果」的
    /// 孤立行（D27 的成因）。有占位行则原地回填，没有才插入新行（存量会话）。
    async fn append_tool_messages(
        &self,
        reqs: Vec<AppendToolMessageReq>,
    ) -> Result<Vec<String>, StoreError>;

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
    /// 环境事实（I38d §4.9.1）：只承载取值，渲染归 `prompt_builder`，内核不读配置。
    pub prompt_facts: PromptFacts,
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

/// 停止钩子判定所需的只读快照。由执行器在两次模型调用之间构造，
/// 只含计数与预算，不携带消息体，避免钩子做任何 IO。
#[derive(Debug, Clone, Copy)]
pub struct TurnState<'a> {
    pub iteration: u32,
    pub max_iterations: u32,
    pub tool_calls: u32,
    pub max_tool_calls: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub elapsed: Duration,
    /// turn 墙钟总时长上限；`None` 或 `0` 表示不设限。由 TurnRunner 下发，执行器只读。
    pub turn_deadline: Option<Duration>,
    /// 本次调用使用的模型名，供按模型差异化判定的钩子使用。
    pub model: &'a str,
}

/// 停止钩子结论。`Continue` 表示继续下一迭代；`Stop` 携带收尾原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopDecision {
    Continue,
    Stop { reason: String },
}

/// 停止钩子。执行器在每次迭代之间按注册顺序调用，任一 `Stop` 即收尾。
/// `check` 必须同步且禁止 IO / await：钩子只读 `TurnState` 快照。
pub trait StopHook: Send + Sync {
    fn name(&self) -> &'static str;

    fn check(&self, state: &TurnState<'_>) -> StopDecision;
}

#[cfg(any(test, feature = "test-doubles"))]
pub mod test_doubles;
