use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use assistant_tools::{ApprovalDecision, ApprovalPrompter, ApprovalRequest};
use async_trait::async_trait;

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
/// 用 `Instant::now()`（真实单调时钟）而不是注入的 `Clock`：测试替身是常量时钟，
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
