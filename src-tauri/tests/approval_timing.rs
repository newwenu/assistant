// 参考: docs/store/I38b-agent-loop-spec.md §6.4 / D12 — 审批等待计时装饰器

use std::sync::Arc;
use std::time::Duration;

use assistant_app::adapters::{ApprovalWaitRegistry, TimingApprovalPrompter};
use assistant_tools::{
    request_with_timeout, ApprovalContext, ApprovalDecision, ApprovalPrompter, ApprovalRequest,
    ApprovalWait, ToolTier,
};
use async_trait::async_trait;

/// 固定等待 20ms 才返回的审批替身：让 `WaitGuard` 有可断言的等待时长。
struct SlowApproval;

#[async_trait]
impl ApprovalPrompter for SlowApproval {
    async fn request(&self, _request: ApprovalRequest) -> ApprovalDecision {
        tokio::time::sleep(Duration::from_millis(20)).await;
        ApprovalDecision::Approve
    }
}

fn request(call_id: &str, timeout: Duration) -> ApprovalRequest {
    ApprovalRequest {
        tool_call_id: call_id.to_string(),
        conversation_id: Some("conv-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        iteration: Some(1),
        tool_name: "echo".to_string(),
        arguments: serde_json::json!({}),
        intents: Vec::new(),
        timeout,
        context: ApprovalContext::minimal(ToolTier::Os, Default::default(), Vec::new()),
    }
}

fn timed(registry: &ApprovalWaitRegistry) -> TimingApprovalPrompter {
    TimingApprovalPrompter::with_registry(Arc::new(SlowApproval), registry.clone())
}

/// C4.34：正常审批路径按 `tool_call_id` 记录等待毫秒数，且 `take` 后即移除。
#[tokio::test]
async fn approval_wait_recorded_per_call_and_removed_after_take() {
    let registry = ApprovalWaitRegistry::new();
    let prompter = timed(&registry);

    let decision = prompter
        .request(request("call-1", Duration::from_secs(5)))
        .await;
    assert_eq!(decision, ApprovalDecision::Approve);

    let waited = registry.peek("call-1").expect("正常路径必须记录等待时长");
    assert!(waited >= 20, "等待时长应不小于替身 sleep：{waited}ms");

    assert_eq!(registry.take("call-1"), Some(waited), "take 返回并移除");
    assert!(registry.is_empty(), "take 之后不得残留，避免跨迭代无界增长");
}

/// 永不返回的审批替身：模拟「无人应答」。
///
/// 不能用 `sleep` 替身：`tokio::time::timeout` 先轮询内部 future、再查截止时间，
/// 整仓并行下任务若被抢占超过「超时 - sleep」的时间，两个计时器同时就绪就会返回
/// `Decided`，把「无人应答」误判成「已决策」。永不就绪的 future 让超时判定与调度无关。
struct NeverApproval;

#[async_trait]
impl ApprovalPrompter for NeverApproval {
    async fn request(&self, _request: ApprovalRequest) -> ApprovalDecision {
        std::future::pending().await
    }
}

/// C4.34：超时由外层 `request_with_timeout` 触发，future 被 drop 时仍记录等待。
#[tokio::test]
async fn approval_wait_recorded_on_timeout_drop() {
    let registry = ApprovalWaitRegistry::new();
    let prompter = TimingApprovalPrompter::with_registry(Arc::new(NeverApproval), registry.clone());

    let decision =
        request_with_timeout(&prompter, request("call-2", Duration::from_millis(5))).await;
    assert!(
        matches!(decision, ApprovalWait::TimedOut { .. }),
        "超时必须归一为拒绝且与用户拒绝区分，不得放行：{decision:?}"
    );

    let waited = registry
        .peek("call-2")
        .expect("drop 路径同样要记录等待时长");
    assert!(waited >= 5, "至少覆盖到超时点：{waited}ms");
    assert_eq!(registry.take("call-2"), Some(waited));
}

/// C4.34：多个调用的等待时长互不覆盖，按 `tool_call_id` 独立记账。
#[tokio::test]
async fn approval_waits_are_keyed_by_call_id() {
    let registry = ApprovalWaitRegistry::new();
    let prompter = timed(&registry);

    prompter
        .request(request("call-a", Duration::from_secs(5)))
        .await;
    prompter
        .request(request("call-b", Duration::from_secs(5)))
        .await;

    assert_eq!(registry.len(), 2, "两个调用各自一条等待记录");
    assert!(registry.peek("call-a").is_some());
    assert!(registry.peek("call-b").is_some());
    assert!(registry.take("call-a").is_some());
    assert_eq!(registry.len(), 1, "只移除被取走的那一条");
}
