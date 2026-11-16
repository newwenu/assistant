use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::context::ApprovalRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayDecision {
    Allowed,
    Denied {
        action: String,
        resource: String,
    },
    /// 用户明确拒绝：模型应据此调整意图，不得重试同一操作。
    RejectedByUser {
        reason: Option<String>,
    },
    /// 审批等待超时（无人应答）：与「用户拒绝」是两件事，模型应提示用户在场，
    /// 而不是解释「你拒绝了」。审计与错误类型也随之区分。
    ApprovalTimedOut {
        timeout: Duration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Deny {
        reason: Option<String>,
    },
    /// 仅当前会话内同键不再询问。
    AllowSession,
    /// 所有会话内同键不再询问。
    AllowAlways,
}

/// 一次审批等待的结局。
///
/// 「超时」不是用户的决定，因此**不**进入 [`ApprovalDecision`] 的值域：
/// prompter 只能表达用户的选择，等待被外力结束由本枚举单独表达。
/// 这样下游（网关 / 审计 / 回填模型的文案）才能区分「没人应答」与「用户拒绝」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalWait {
    Decided(ApprovalDecision),
    TimedOut { timeout: Duration },
}

#[async_trait]
pub trait ApprovalPrompter: Send + Sync {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision;
}

/// 审批等待的唯一入口：超时归一为拒绝（fail-closed），但**保留「为何结束」**。
///
/// 超时绝不默认放行，也绝不无限挂起；同时不把超时伪装成用户拒绝，
/// 由 [`ApprovalWait`] 把两者分开交给调用方。
pub async fn request_with_timeout(
    prompter: &dyn ApprovalPrompter,
    request: ApprovalRequest,
) -> ApprovalWait {
    let timeout = request.timeout;
    match tokio::time::timeout(timeout, prompter.request(request)).await {
        Ok(decision) => ApprovalWait::Decided(decision),
        Err(_) => ApprovalWait::TimedOut { timeout },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditOutcome {
    /// 策略静态允许。
    Allowed,
    /// 命中审批缓存：用户此前已授权该规范化键。
    GrantHit,
    Denied,
    Approved,
    /// 本会话内授权。
    SessionApproved,
    AlwaysApproved,
    /// 用户明确拒绝。
    Rejected,
    /// 审批等待超时：用户没做决定，与 `Rejected` 分开统计（超时率是 UX 指标）。
    TimedOut,
    /// 入参校验失败：工具参数未通过自身校验，未进入策略求值。
    ValidateRejected,
    /// `permission_intents` 求值失败：工具无法声明权限意图。
    IntentError,
    /// **未经权限策略求值即执行**：只允许「空 intent 短路」与「ToolBox 直调」两个触发点，
    /// 不得用它表示「策略允许」（策略允许是 `Allowed`）。
    Bypassed,
}

/// 审批超时的取值来源：每次审批都重新求值，配置改动对下一轮立即生效。
pub type ApprovalTimeoutSource = std::sync::Arc<dyn Fn() -> Duration + Send + Sync>;
