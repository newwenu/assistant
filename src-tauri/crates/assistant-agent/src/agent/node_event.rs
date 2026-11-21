//! 节点级观测词汇（I55 §3.1 / §4.2 / §4.3 / §4.4）。
//!
//! 这里只定义「turn 模板」的 12 个节点、7 值节点状态与按节点分派的 detail 闭集；
//! 发声点在各执行器内（V1-b），wire 投影在 `assistant-protocol` 的 `agent.rs`（V1-d）。
//!
//! 命名纪律：本模板是**编译期常量**（「turn 模板」），不进 wire、不由后端下发；
//! 将来用户可编辑的图是**数据**（「工作流模板」），独立立项，两者禁止共用类型名。

use serde::Serialize;

use super::round_executor::ToolStage;

/// 当前唯一实例的 agent 标识（I55 N17）：子代理 / 编排器接入后非 root。
pub const ROOT_AGENT_ID: &str = "root";

/// turn 模板的 12 个节点（I55 §3.1），wire 序列化为 `kebab-case`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentNode {
    Admit,
    Prepare,
    Brake,
    Ladder,
    Model,
    PersistAssistant,
    Branch,
    BudgetSplit,
    Pipeline,
    AfterTool,
    PersistTools,
    AfterIteration,
}

/// 节点状态 7 值闭集（I55 §4.3）；终态只有 `finished` / `error` / `skipped` / `cancelled`。
///
/// `pending` 与 `skipped` 不由后端发（§4.6 第 1–2 条）：前者由静态模板推导，后者由 `branch`
/// 的 `finished` 批量标记。后端只发 `running` / `waiting` / 三个终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Pending,
    Running,
    Waiting,
    Finished,
    Error,
    Skipped,
    Cancelled,
}

/// 单条节点事件（I55 §4.2 全形）。
///
/// `seq` 与 turn 内的 `Delta` 共用同一单调序列（`TurnEmitter` 的计数器），两者混排才能
/// 断言「第 N 条增量落在 `model` 的 `running` 期内」。`at_ms` 由调用方用 `Clock` 端口算出，
/// 内核禁止 `SystemTime::now()`。
///
/// `transition` 是写侧语义的显式声明：`true` 只用于 §5.1 迁移表的合法迁移，`false` 只用于
/// 「节点内部子阶段进入」这类过程事实（只进时间线，不改格子状态、不重置 `sinceMs`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNodeEvent {
    pub turn_id: String,
    pub conversation_id: String,
    pub seq: u32,
    pub node: AgentNode,
    /// 迭代内节点从 1 起；迭代外节点（`admit` / `prepare`）恒为 0。
    pub iteration: u32,
    pub state: NodeState,
    pub transition: bool,
    pub at_ms: u64,
    pub agent_id: String,
    pub parent_turn_id: Option<String>,
    pub detail: Option<AgentNodeDetail>,
}

/// 按节点分派的 detail 闭集（I55 §4.4，只含 M 级字段）。
///
/// `admit` / `prepare` 无 detail（`None`）；`persist-assistant` 与 `persist-tools` 共用
/// `Persist` 形态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AgentNodeDetail {
    Brake(BrakeDetail),
    Ladder(LadderDetail),
    Model(ModelDetail),
    Pipeline(PipelineDetail),
    BudgetSplit(BudgetSplitDetail),
    Branch(BranchDetail),
    Persist(PersistDetail),
    AfterTool(AfterToolDetail),
    AfterIteration(AfterIterationDetail),
}

/// 刹车判定（I55 §5.1 末条）：`stop` 是正常控制流，节点仍是 `finished`，不是 `error`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BrakeVerdict {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrakeDetail {
    pub verdict: BrakeVerdict,
    /// 命中钩子名；`verdict: continue` 时为 `None`。
    pub hook: Option<String>,
    pub budget: BudgetRemaining,
}

/// 剩余预算（I55 §4.4 末段）：每个字段都是 `Option`，`None` 表示「未配置」而非 0。
///
/// 未配置显示成 0 会被读成「已用尽」，与 I54 §1.2 第 6 条的资源显示同一口径。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetRemaining {
    pub remaining_iterations: Option<u32>,
    pub remaining_tool_calls: Option<u32>,
    pub remaining_wall_clock_ms: Option<u64>,
}

/// 上下文阶梯：`applied` 只记**实际改写过请求**的中间件名（I55 Q2）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LadderDetail {
    pub applied: Vec<String>,
    pub dropped_messages: Option<u32>,
    pub trimmed_chars: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetail {
    /// 本次迭代的模型调用序号（含内层重试），从 1 起。
    pub attempt: u32,
    pub retry_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub tool_call_count: u32,
}

/// 工具流水线节点（I55 §4.4）：有两种形态，**不得混用**。
///
/// - **过程形态**（随 `transition: false` 的 `StageEntered`）：填 `stage` / `index` / `total` /
///   `tool_name` / `tool_call_id`；
/// - **终态形态**（随 `transition: true` 的 `running` / `finished` / `error` / `cancelled`）：
///   全字段 `None`——一次执行有多个子阶段与多个并发调用，塞进单值字段只会得到「最后一个」。
///
/// `stage` 直接复用 `ToolStage`（5 值），不新造枚举、不新造第二份镜像。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDetail {
    pub stage: Option<ToolStage>,
    /// 按发起顺序的全局序号（I55 Q3）。
    pub index: Option<u32>,
    pub total: Option<u32>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSplitDetail {
    pub requested: u32,
    pub executed: u32,
    pub deferred: u32,
}

/// 判定节点：无工具调用时 `finished` 之后立即出口 `completed`，前端据此把 ⑧–⑫ 标 `skipped`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetail {
    pub has_tool_calls: bool,
    pub wrap_up: bool,
    pub tool_call_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistDetail {
    pub rows: u32,
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AfterToolDetail {
    pub verdicts: Vec<MiddlewareVerdict>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewareVerdict {
    pub middleware: String,
    pub action: MiddlewareAction,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MiddlewareAction {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AfterIterationDetail {
    pub nudges: Vec<String>,
    pub next_iteration: u32,
}
