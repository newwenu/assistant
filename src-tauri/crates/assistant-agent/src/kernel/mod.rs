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
    AppendToolMessageReq, ChatError, Clock, CompactionSink, CreateAssistantReq, EventSink,
    InputError, MessageStatus, MessageStore, ProviderCredential, ProviderDelta, StopHook,
    StoreError, ToolCatalog, TurnEvent, TurnInput, TurnInputSource, UsageRecord,
};
pub use request::ChatRequest;
pub use runner::{CompactionConfig, TurnRunner};
pub use state::{SessionStates, StateError};
pub use stream::{PartialOutcome, StreamError, StreamOutcome, UsageSnapshot};
pub use tasks::{Busy, TurnEntry, TurnTasks};

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

    /// agent 链路：拒绝、12 迭代、工具目录由宿主注入、全阶梯、广播路由。
    pub fn agent() -> Self {
        Self {
            on_busy: OnBusy::Reject,
            max_iterations: limits::DEFAULT_MAX_ITERATIONS,
            tools: None,
            ladder: LadderPolicy::Full,
            stop_hooks: Vec::new(),
            wrap_up: true,
            route: EventRoute::Broadcast,
            kind: "agent",
            limits: TurnLimits::default(),
        }
    }
}
