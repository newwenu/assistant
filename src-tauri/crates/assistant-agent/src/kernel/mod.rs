use std::sync::Arc;

pub mod emit;
pub mod ids;
pub mod limits;
pub mod outcome;
pub mod persist;
pub mod phase;
pub mod ports;
pub mod request;
pub mod runner;
pub mod state;
pub mod stream;
pub mod tasks;

pub use emit::TurnEmitter;
pub use ids::{TurnContext, TurnId};
pub use limits::TurnLimits;
pub use outcome::{TurnError, TurnOutcome, TurnStatus};
pub use persist::{PersistCtx, TurnPersister};
pub use phase::{SessionState, TurnPhase};
pub use ports::{
    ChatError, Clock, CompactionSink, CreateAssistantReq, EventSink, InputError, MessageStatus,
    MessageStore, ProviderCredential, ProviderDelta, StoreError, TurnEvent, TurnInput,
    TurnInputSource, UsageRecord,
};
pub use request::ChatRequest;
pub use runner::{CompactionConfig, TurnRunner};
pub use state::{SessionStates, StateError};
pub use stream::{PartialOutcome, StreamError, StreamOutcome, UsageSnapshot};
pub use tasks::{Busy, TurnEntry, TurnTasks};

/// 工具轮执行端口。P3 恒为 `None`，P4 由宿主注入真实实现；
/// 内核不得 `use` 工具类型，因此这里只保留最小签名。
#[async_trait::async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn run_iterations(&self, ctx: &TurnContext, iteration: u32) -> RoundOutcome;
}

/// 一次工具轮的结果。P4 细化字段，P3 只用于占位。
#[derive(Debug, Clone, Default)]
pub struct RoundOutcome {
    pub tool_calls: u32,
    pub executed: bool,
}

/// 工具目录端口。P3 只做占位，P4（I38b §4.1）以 `ToolSpec` 填充签名，
/// 实现放在 `src-tauri/src/adapters/`，内核不得 import `assistant-tools`。
pub trait ToolCatalog: Send + Sync {}

/// 停止钩子。P3 只做占位，P4（I36 Step 5）实现 `check`，`check` 必须是同步且禁止 IO。
pub trait StopHook: Send + Sync {
    fn name(&self) -> &'static str;
}

/// P3 占位实现：空目录。P4 由适配层注入真实 `ToolCatalog`。
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyToolCatalog;

impl ToolCatalog for EmptyToolCatalog {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnBusy {
    /// chat：取消旧令牌并递增，静默顶替。
    Replace,
    /// agent：返回 `Busy`，不动旧令牌。
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRoute {
    Orb,
    Broadcast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderPolicy {
    /// 仅投影 + 终止式压缩（chat）。
    ProjectionOnly,
    /// 投影 + 终止式压缩（语义同 `ProjectionOnly`，独立取值便于后续区分）。
    TerminatingCompact,
    /// 全阶梯（agent）。
    Full,
}

pub struct TurnSpec {
    pub on_busy: OnBusy,
    pub max_iterations: u32,
    pub tools: Option<Arc<dyn ToolCatalog>>,
    pub ladder: LadderPolicy,
    pub stop_hooks: Vec<Arc<dyn StopHook>>,
    pub wrap_up: bool,
    pub route: EventRoute,
    /// usage 记录的 `kind` 分类。与 `route` 解耦：路由决定事件投递范围，
    /// `kind` 决定统计口径，二者未来可能不同（如 agent 走 orb 路由）。
    pub kind: &'static str,
    /// 本轮预算。链路差异只通过它表达，`TurnRunner` 不再单独收 limits。
    pub limits: TurnLimits,
}

impl TurnSpec {
    /// chat 链路：顶替、1 迭代、无工具、仅投影 + 终止式压缩、orb 路由。
    pub fn chat() -> Self {
        Self {
            on_busy: OnBusy::Replace,
            max_iterations: 1,
            tools: None,
            ladder: LadderPolicy::ProjectionOnly,
            stop_hooks: Vec::new(),
            wrap_up: false,
            route: EventRoute::Orb,
            kind: "chat",
            limits: TurnLimits::chat(),
        }
    }

    /// agent 链路：拒绝、12 迭代、带工具、全阶梯、广播路由。
    pub fn agent() -> Self {
        Self {
            on_busy: OnBusy::Reject,
            max_iterations: limits::DEFAULT_MAX_ITERATIONS,
            tools: Some(Arc::new(EmptyToolCatalog)),
            ladder: LadderPolicy::Full,
            stop_hooks: Vec::new(),
            wrap_up: true,
            route: EventRoute::Broadcast,
            kind: "agent",
            limits: TurnLimits::default(),
        }
    }
}
