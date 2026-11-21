use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use assistant_tools::{ApprovalDecision, ApprovalPrompter, ApprovalRequest};
use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{AgentToolCallDto, AgentToolCallPayloadDto, ToolApprovalRequestDto};

/// P4 的审批通道默认实现：**没有 UI 就一律拒绝**（fail-closed）。
///
/// `ResolvedPermissionPolicy` 判为 `Ask` 的调用在此直接 `Deny`，绝不放行；
/// 该决策的审计由 `ToolGateway` 在 `Deny` 分支写 `permission_audit`（`Rejected`），
/// 本实现不重复写，避免同一决策两条审计记录。
/// 真实审批 UI 归 P6：届时只替换本内层，`TimingApprovalPrompter` 装饰器结构不变。
pub const APPROVAL_UNAVAILABLE_REASON: &str = "approval channel not available in P4";

#[derive(Debug, Default, Clone, Copy)]
pub struct DenyAllApprovalPrompter;

impl DenyAllApprovalPrompter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ApprovalPrompter for DenyAllApprovalPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        log::warn!(
            "approval unavailable in P4, denying '{}' (tool_call_id='{}')",
            request.tool_name,
            request.tool_call_id
        );
        ApprovalDecision::Deny {
            reason: Some(APPROVAL_UNAVAILABLE_REASON.to_string()),
        }
    }
}

/// 审批等待时长的唯一事实源：`tool_call_id -> ms`（§6.4 / D12）。
///
/// 只由 `TimingApprovalPrompter` 的 `WaitGuard` 写入；`ToolExecutionPort` 适配器
/// 在 `pipeline.execute` 返回后 `take` 并填进 `ToolExecResult`，随即从表里移除，
/// 保证跨迭代不会无界增长。该值不落库，只进事件与 `RoundOutcome` 汇总。
#[derive(Debug, Clone, Default)]
pub struct ApprovalWaitRegistry {
    waits: Arc<Mutex<HashMap<String, u64>>>,
}

impl ApprovalWaitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, call_id: String, elapsed_ms: u64) {
        match self.waits.lock() {
            Ok(mut waits) => {
                waits.insert(call_id, elapsed_ms);
            }
            Err(_) => log::warn!("approval wait registry poisoned, dropping wait duration"),
        }
    }

    /// 事件发射时读值但**不**移除，移除交给 `take`，避免事件与结果两条路径互相抢。
    pub fn peek(&self, call_id: &str) -> Option<u64> {
        self.waits
            .lock()
            .ok()
            .and_then(|waits| waits.get(call_id).copied())
    }

    pub fn take(&self, call_id: &str) -> Option<u64> {
        self.waits
            .lock()
            .ok()
            .and_then(|mut waits| waits.remove(call_id))
    }

    pub fn len(&self) -> usize {
        self.waits.lock().map(|waits| waits.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 审批计时装饰器：包住任意 `ApprovalPrompter`，按 `tool_call_id` 记录等待毫秒数。
///
/// 用 `Instant::now()`（真实单调时间源）而不是注入的时间源抽象：测试替身是常量源，
/// 差值恒为 0，无法断言。超时由 `ToolGateway` 外层的 `request_with_timeout` 触发，
/// 会把本 future drop 掉，所以计时必须放在 `Drop` 守卫里 —— 正常审批与超时两条
/// 路径都能拿到等待时长，且不必改动 `assistant-tools` 的超时归属。
pub struct TimingApprovalPrompter {
    inner: Arc<dyn ApprovalPrompter>,
    waits: ApprovalWaitRegistry,
}

impl TimingApprovalPrompter {
    pub fn new(inner: Arc<dyn ApprovalPrompter>) -> Self {
        Self::with_registry(inner, ApprovalWaitRegistry::new())
    }

    pub fn with_registry(inner: Arc<dyn ApprovalPrompter>, waits: ApprovalWaitRegistry) -> Self {
        Self { inner, waits }
    }

    pub fn waits(&self) -> &ApprovalWaitRegistry {
        &self.waits
    }
}

#[async_trait]
impl ApprovalPrompter for TimingApprovalPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        let _guard = WaitGuard {
            call_id: request.tool_call_id.clone(),
            started: Instant::now(),
            waits: self.waits.clone(),
        };
        self.inner.request(request).await
    }
}

struct WaitGuard {
    call_id: String,
    started: Instant,
    waits: ApprovalWaitRegistry,
}

impl Drop for WaitGuard {
    fn drop(&mut self) {
        let elapsed_ms = self.started.elapsed().as_millis() as u64;
        self.waits.record(self.call_id.clone(), elapsed_ms);
    }
}

/// 待审批登记表：`call_id -> (conversation_id, 决策投递端)`。
///
/// 审批是「网关 await、UI 回填」的双向握手，因此必须有一个按 `call_id` 定位的
/// 挂起点。键用 `call_id` 而不是 `(conversation, call)`：`call_id` 由模型生成且
/// 在一次 turn 内唯一，重复即视为异常（fail-closed 拒绝，不覆盖已有挂起点）。
/// 同时记录 `conversation_id`，回填时校验，避免 A 会话的窗口关掉 B 会话的卡片。
#[derive(Debug, Clone, Default)]
pub struct PendingApprovalRegistry {
    pending: Arc<Mutex<HashMap<String, PendingApproval>>>,
}

#[derive(Debug)]
struct PendingApproval {
    conversation_id: String,
    sender: oneshot::Sender<ApprovalDecision>,
}

impl PendingApprovalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个挂起点。已有同 `call_id` 的挂起点时返回 `None`（调用方 fail-closed）。
    fn register(
        &self,
        call_id: &str,
        conversation_id: &str,
    ) -> Option<oneshot::Receiver<ApprovalDecision>> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.contains_key(call_id) {
            return None;
        }
        pending.insert(
            call_id.to_string(),
            PendingApproval {
                conversation_id: conversation_id.to_string(),
                sender: tx,
            },
        );
        Some(rx)
    }

    /// 投递决策。返回 `false` 表示该 `call_id` 已不在等待中（超时 / 已决策 / 未知）
    /// 或 `conversation_id` 不匹配（跨会话误投，按未找到处理，不泄漏挂起点状态）。
    pub fn resolve(
        &self,
        call_id: &str,
        conversation_id: &str,
        decision: ApprovalDecision,
    ) -> bool {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let matches = pending
            .get(call_id)
            .is_some_and(|entry| entry.conversation_id == conversation_id);
        if !matches {
            return false;
        }
        match pending.remove(call_id) {
            Some(entry) => entry.sender.send(decision).is_ok(),
            None => false,
        }
    }

    fn discard(&self, call_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(call_id);
    }

    pub fn len(&self) -> usize {
        self.pending
            .lock()
            .map(|pending| pending.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 挂起点的清理守卫：网关超时或 turn 被取消时 future 被 drop，登记项必须一并移除，
/// 否则登记表会随超时次数无界增长。正常决策走 `resolve` 已先移除，此处为幂等空操作。
///
/// 除清理外还负责**撤回卡片**：没有决策就离开 `request` 只有一种可能——
/// future 被 drop（网关超时或 turn 取消）。此时必须发 `ApprovalClosed`，
/// 否则前端卡片会永远停在「等待审批」，用户回来点按钮只会拿到「无挂起点」错误。
struct PendingGuard<'a> {
    registry: &'a PendingApprovalRegistry,
    call_id: String,
    closed: Option<ClosedApproval>,
    decided: bool,
}

/// 撤回事件所需的全部信息：`Drop` 里不能再借用 `request`，必须提前克隆。
struct ClosedApproval {
    app: tauri::AppHandle,
    conversation_id: String,
    turn_id: String,
    iteration: u32,
    call_id: String,
    tool_name: String,
    timeout_ms: u64,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.registry.discard(&self.call_id);
        if self.decided {
            return;
        }
        let Some(closed) = self.closed.take() else {
            return;
        };
        emit_broadcast(
            &closed.app,
            event_name::AGENT_TOOL_CALL,
            AgentToolCallDto {
                conversation_id: closed.conversation_id,
                turn_id: closed.turn_id,
                iteration: closed.iteration,
                call_id: closed.call_id,
                name: closed.tool_name,
                payload: AgentToolCallPayloadDto::ApprovalClosed {
                    timeout_ms: closed.timeout_ms,
                },
            },
        );
    }
}

/// 真实审批通道：把 `Ask` 转成前端卡片，等用户点 [允许] / [拒绝] / [本会话允许] / [始终允许]。
///
/// 与 `DenyAllApprovalPrompter` 的差别只在「决策从哪来」：决策来源换成 IPC 回填，
/// 超时仍由 `ToolGateway` 的 `request_with_timeout` 负责（超时即 `Deny`）。
/// 缺路由信息（无 `conversationId` / `turnId`）时**不发事件直接拒绝**：
/// 卡片无法被路由到任何窗口，等下去只会把 turn 挂死到超时。
pub struct TauriApprovalPrompter {
    app: tauri::AppHandle,
    pending: PendingApprovalRegistry,
}

impl TauriApprovalPrompter {
    pub fn new(app: tauri::AppHandle, pending: PendingApprovalRegistry) -> Self {
        Self { app, pending }
    }

    pub fn pending(&self) -> &PendingApprovalRegistry {
        &self.pending
    }

    fn deny(reason: &str) -> ApprovalDecision {
        ApprovalDecision::Deny {
            reason: Some(reason.to_string()),
        }
    }
}

#[async_trait]
impl ApprovalPrompter for TauriApprovalPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        let (conversation_id, turn_id) =
            match (request.conversation_id.clone(), request.turn_id.clone()) {
                (Some(conversation_id), Some(turn_id)) => (conversation_id, turn_id),
                _ => {
                    log::error!(
                        "approval request for '{}' lacks conversation/turn id, denying",
                        request.tool_name
                    );
                    return Self::deny("approval request lacks conversation or turn id");
                }
            };

        let receiver = match self
            .pending
            .register(&request.tool_call_id, &conversation_id)
        {
            Some(receiver) => receiver,
            None => {
                log::error!(
                    "duplicate pending approval for call_id='{}', denying",
                    request.tool_call_id
                );
                return Self::deny("duplicate pending approval for this call");
            }
        };
        let mut guard = PendingGuard {
            registry: &self.pending,
            call_id: request.tool_call_id.clone(),
            closed: Some(ClosedApproval {
                app: self.app.clone(),
                conversation_id: conversation_id.clone(),
                turn_id: turn_id.clone(),
                iteration: request.iteration.unwrap_or_default(),
                call_id: request.tool_call_id.clone(),
                tool_name: request.tool_name.clone(),
                timeout_ms: request.timeout.as_millis() as u64,
            }),
            decided: false,
        };

        let approval = ToolApprovalRequestDto {
            conversation_id: conversation_id.clone(),
            turn_id: turn_id.clone(),
            call_id: request.tool_call_id.clone(),
            tool_name: request.tool_name.clone(),
            arguments: request.arguments.clone(),
            intents: request
                .intents
                .clone()
                .into_iter()
                .map(Into::into)
                .collect(),
            timeout_ms: request.timeout.as_millis() as u64,
            context: request.context.clone().into(),
        };
        emit_broadcast(
            &self.app,
            event_name::AGENT_TOOL_CALL,
            AgentToolCallDto {
                conversation_id,
                turn_id,
                iteration: request.iteration.unwrap_or_default(),
                call_id: request.tool_call_id.clone(),
                name: request.tool_name.clone(),
                payload: AgentToolCallPayloadDto::AwaitingApproval { approval },
            },
        );

        let decision = match receiver.await {
            Ok(decision) => decision,
            Err(_) => Self::deny("approval channel closed before a decision arrived"),
        };
        // 走到这里说明等待已正常结束（无论拿到决策还是通道关闭），
        // 撤回事件不应再发，否则会把已决策的卡片又标成失效。
        guard.decided = true;
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只验证清理语义：`PendingGuard` 的撤回事件依赖 `tauri::AppHandle`，
    /// 单元测试不构造它，直接测它调用的 `discard`。
    fn register(registry: &PendingApprovalRegistry, call_id: &str, conversation_id: &str) {
        registry
            .register(call_id, conversation_id)
            .expect("首次登记必须成功");
    }

    #[test]
    fn resolve_requires_matching_conversation() {
        let registry = PendingApprovalRegistry::new();
        let _rx = registry.register("call-1", "conv-a").expect("首次登记");

        assert!(
            !registry.resolve("call-1", "conv-b", ApprovalDecision::Approve),
            "跨会话回填不得投递"
        );
        assert_eq!(
            registry.len(),
            1,
            "误投不得移除挂起点，否则原会话永远等不到"
        );

        assert!(
            registry.resolve("call-1", "conv-a", ApprovalDecision::Approve),
            "正确会话必须能投递"
        );
        assert!(registry.is_empty(), "投递后挂起点立即移除");
    }

    #[test]
    fn duplicate_call_id_is_refused_and_keeps_original() {
        let registry = PendingApprovalRegistry::new();
        let _first = registry.register("call-1", "conv-a").expect("首次登记");

        assert!(
            registry.register("call-1", "conv-a").is_none(),
            "重复 call_id 必须 fail-closed 拒绝，不得覆盖已有挂起点"
        );
        assert_eq!(registry.len(), 1, "拒绝不得影响原挂起点");
    }

    #[test]
    fn resolve_unknown_call_id_reports_false() {
        let registry = PendingApprovalRegistry::new();
        assert!(!registry.resolve("missing", "conv-a", ApprovalDecision::Approve));
    }

    #[test]
    fn resolve_is_not_repeatable() {
        let registry = PendingApprovalRegistry::new();
        let _rx = registry.register("call-1", "conv-a").expect("首次登记");

        assert!(registry.resolve("call-1", "conv-a", ApprovalDecision::Approve));
        assert!(
            !registry.resolve("call-1", "conv-a", ApprovalDecision::Deny { reason: None }),
            "同一 call_id 只能决策一次，重复回填必须无效"
        );
    }

    #[test]
    fn discard_removes_pending_entry() {
        let registry = PendingApprovalRegistry::new();
        register(&registry, "call-1", "conv-a");

        registry.discard("call-1");
        assert!(
            registry.is_empty(),
            "超时 / 取消路径必须清理登记项，否则登记表无界增长"
        );
    }

    #[test]
    fn discard_after_resolve_is_idempotent() {
        let registry = PendingApprovalRegistry::new();
        let _rx = registry.register("call-1", "conv-a").expect("首次登记");
        assert!(registry.resolve("call-1", "conv-a", ApprovalDecision::Approve));

        registry.discard("call-1");
        assert!(registry.is_empty(), "正常决策路径的清理必须是幂等空操作");
    }

    #[test]
    fn discard_does_not_touch_other_calls() {
        let registry = PendingApprovalRegistry::new();
        register(&registry, "call-1", "conv-a");
        let _second = registry.register("call-2", "conv-a").expect("首次登记");

        registry.discard("call-1");
        assert_eq!(registry.len(), 1, "清理必须按 call_id 精确移除");
        assert!(registry.resolve("call-2", "conv-a", ApprovalDecision::Approve));
    }
}
