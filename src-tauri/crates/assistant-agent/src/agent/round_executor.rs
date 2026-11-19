use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use assistant_providers::base::ProviderErrorPayload;

use crate::kernel::{
    Clock, MessageStore, ProviderDelta, TurnError, TurnGeneration, TurnInput, TurnLimits,
    TurnPhase, TurnSpec, TurnStatus, UsageSnapshot,
};

use super::node_event::{AgentNode, AgentNodeDetail, NodeState};
pub use super::tool_call::FinalizedToolCall;
use super::tool_event::ToolEvent;

#[async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn run_iterations(&self, ctx: TurnRunCtx<'_>) -> RoundOutcome;
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub status: TurnStatus,
    pub text: String,
    pub reasoning: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub attempt_usages: Vec<UsageSnapshot>,
    pub ttft_ms: Option<u64>,
    pub confirmation_wait_ms: u64,
    pub hit_cap: bool,
    pub wrap_up_injected: bool,
    /// 本 turn 最后一条被写入并 `finalize` 的 assistant 行 id（§5.1）。
    pub final_message_id: Option<String>,
    /// 最后一次实际发出请求的 `context_window_tokens`（压缩判定的输入）。
    pub context_window_tokens: Option<u32>,
    /// 最后一次实际发出请求的 `ChatOptions.max_tokens`（压缩判定的 reserved 输入）。
    pub reserved_output_tokens: u32,
    /// 终态错误的具名出口（内核内部口径，不参与 wire）。
    pub error: Option<TurnError>,
    /// provider 失败时的**原始**错误载荷（`code` / `retryable` / `retry_after_ms` 等），
    /// 供 `TurnRunner` 还原 `TurnEvent::Terminated.error`（wire 契约 I36 §4）。
    /// `TurnError::Provider` 只留文案，无法承载这些字段，故单列。
    pub provider_error: Option<ProviderErrorPayload>,
}

impl Default for RoundOutcome {
    fn default() -> Self {
        Self {
            status: TurnStatus::Completed,
            text: String::new(),
            reasoning: String::new(),
            iterations: 0,
            model_calls: 0,
            tool_calls: 0,
            prompt_tokens: None,
            completion_tokens: None,
            cache_read_tokens: None,
            total_tokens: None,
            attempt_usages: Vec::new(),
            ttft_ms: None,
            confirmation_wait_ms: 0,
            hit_cap: false,
            wrap_up_injected: false,
            final_message_id: None,
            context_window_tokens: None,
            reserved_output_tokens: 0,
            error: None,
            provider_error: None,
        }
    }
}

pub trait TurnObserver: Send + Sync {
    fn set_phase(&self, phase: TurnPhase);

    /// 进入新迭代时同步迭代号。`Delta` / `ToolEvent` 据此标记所属迭代，
    /// 前端以它为气泡边界（I38d §3.1.1：一次迭代 = 一条消息 = 一个气泡）。
    fn set_iteration(&self, iteration: u32);

    fn emit_tool_event(&self, event: ToolEvent);

    /// 正文 / 推理增量：返回本次事件的 `seq`。executor **只能**经这里发 delta，
    /// 不得直接持有 `EventSink`（seq 单调、代际校验、`iteration` 标记都在 `TurnEmitter`）。
    fn delta(&self, delta: &ProviderDelta) -> u32;

    /// 节点级观测（I55 §4.2）。无返回值（G11 同口径）：观测不得成为控制通道。
    ///
    /// `iteration` 由调用方显式传入，**不复用** `set_iteration` 的当前值：迭代外节点
    /// （`admit` / `prepare`）恒为 0。`transition` 同样是显式声明（`true` = 构成状态迁移，
    /// `false` = 仅过程事实），实现方不得自行推断。
    ///
    /// 默认空实现：非广播链路与测试替身不需要节点事件，避免为「不观测」的路径强加噪声。
    fn node(
        &self,
        node: AgentNode,
        iteration: u32,
        state: NodeState,
        transition: bool,
        detail: Option<AgentNodeDetail>,
    ) {
        let _ = (node, iteration, state, transition, detail);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnBudget {
    pub remaining_tool_calls: u32,
    /// **绝对截止时刻**（不是总时长上限），裁剪时与 `clock.now()` 做差。
    pub turn_deadline: Option<Instant>,
}

/// 执行器一次运行所需的全部端口与只读快照；字段与 I38b spec §2.3 的表 1:1。
pub struct TurnRunCtx<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    /// 命令层预建的占位行，第 1 迭代复用（§5.1）。
    pub initial_message_id: &'a str,
    /// 会话内 turn 序号（I46 §2.5）：只搬运，不参与任何判定。
    pub turn_index: Option<u32>,
    /// 起点迭代号，P4 恒为 1。
    pub start_iteration: u32,
    pub remaining_turn_budget: TurnBudget,
    /// turn 起点的 `Instant`，`elapsed` 的唯一算法来源。
    pub turn_started_at: Instant,
    pub child_token: &'a CancellationToken,
    /// 本轮可注入的工具清单（接入层已收敛白名单）。空集视为无工具。
    pub tools: Option<&'a [assistant_providers::types::ToolSpec]>,
    pub tool_exec: Option<&'a dyn ToolExecutionPort>,
    pub observer: &'a dyn TurnObserver,
    pub store: &'a dyn MessageStore,
    /// 已取好的会话快照，**不是**端口：取数失败路径留在 `TurnRunner`。
    pub input: &'a TurnInput,
    pub clock: &'a dyn Clock,
    /// 落库前的代际校验（§5.1）。
    pub generation: &'a dyn TurnGeneration,
    pub limits: &'a TurnLimits,
    pub spec: &'a TurnSpec,
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    /// 本次 turn 使用的模型名（`TurnState.model` 的唯一来源）。
    pub model: &'a str,
    /// usage 记录的分支外键（会话活跃叶）。
    pub branch_leaf_id: Option<&'a str>,
    /// usage 记录的缓存能力标记（按 provider 种类判定）。
    pub cache_capable: bool,
    pub agent_id: &'a str,
    pub parent_turn_id: Option<&'a str>,
    pub delegation_depth: u32,
}

#[async_trait]
pub trait ToolExecutionPort: Send + Sync {
    async fn execute(
        &self,
        calls: Vec<FinalizedToolCall>,
        ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult>;
}

pub struct ToolExecCtx<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    pub iteration: u32,
    pub child_token: &'a CancellationToken,
    pub clock: &'a dyn Clock,
    /// 本轮真实工具集（`TurnRunCtx::tools` 原样透传，I58 §3.1）。
    ///
    /// 供适配器填 `ToolUseContext::available_tool_names`：`request_tool` 的清单
    /// 必须以「模型这次真能调什么」为准，而不是注册表全集。
    pub tools: Option<&'a [assistant_providers::types::ToolSpec]>,
    /// 工具事件的唯一出口。适配器的 `PipelineEventSink` 只转发 P2 的
    /// `Started` / `Finished` / `Failed`（§6.1），不在此处发 `ArgsDelta`，
    /// 也不得绕过 `TurnObserver` 另开事件通道。
    pub observer: &'a dyn TurnObserver,
    /// 轮内升级收集器（I58 §3.8 ②）。`None` 表示本链路不支持升级：适配器必须
    /// 让 `ToolUseContext::escalation` 保持 `None`，`request_tool(enable)` 据此返回业务错误。
    ///
    /// 用 `Arc` 而不是借用：`ToolUseContext::escalation` 是 `Arc<dyn ToolEscalationPort>`
    /// （无生命周期参数），端口要把它带进工具调用就必须能 'static 持有。
    pub escalations: Option<Arc<dyn EscalationSink>>,
}

/// 轮内升级收集器（I58 §3.8 ②）：只收成品 `ToolSpec`，不感知注册表与准入。
///
/// 生命周期是「一个迭代」：编排层每迭代新建一个，工具批次结束后 `drain` 并尾部追加，
/// 下一次模型调用才可见（G21）。跨轮不携带。
pub trait EscalationSink: Send + Sync {
    /// 登记一个已通过准入、已转成 provider 形状的工具声明。
    fn push(&self, tool: assistant_providers::types::ToolSpec);

    /// 取出本次批次登记的全部声明（已清空收集器）。
    fn drain(&self) -> Vec<assistant_providers::types::ToolSpec>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecResult {
    pub call_id: String,
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
    pub stage: ToolStage,
    pub error_kind: Option<ToolErrorClass>,
    pub confirmation_wait_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolStage {
    Find,
    Validate,
    Gateway,
    Execute,
    Persist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolErrorClass {
    Business,
    Timeout,
    Cancelled,
    Internal,
    Denied,
}
