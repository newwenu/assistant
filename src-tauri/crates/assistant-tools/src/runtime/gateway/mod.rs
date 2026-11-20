mod approval_key;
mod context;
mod decision;
mod stores;
mod testing;

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::context::ToolUseContext;
use crate::policy::{PermissionEffect, ResolvedPermissionPolicy};
use crate::r#trait::Tool;

pub use approval_key::{ApprovalKey, ApprovalScope};
pub use context::{
    ApprovalContext, ApprovalExtension, ApprovalExtensionSeverity, ApprovalReason, ApprovalRequest,
    APPROVAL_PREVIEW_BUDGET,
};
pub use decision::{
    request_with_timeout, ApprovalDecision, ApprovalPrompter, ApprovalTimeoutSource, ApprovalWait,
    AuditOutcome, GatewayDecision,
};
pub use stores::{
    redact_resource, redact_secrets, AuditEntry, ChildGrantStore, GrantScope, InMemoryAuditStore,
    InMemoryGrantStore, PermissionAuditStore, PermissionGrantStore,
};
pub use testing::ScriptedApprovalPrompter;

use context::{build_approval_context, merge_pending, PendingApproval};
use decision::AuditOutcome as Outcome;
use stores::now_millis;

/// 工具调用网关：把「工具声明的权限意图」转换成 `GatewayDecision`。
///
/// 决策顺序固定（规格 P2-C4）：intents → 逐 resource 查 grant → policy 判定
/// → 收集 Ask → 合并一次审批 → `AllowSession` / `AllowAlways` 落 grant → 审计。
/// 任一 `Deny` 立即返回，绝不因为其它 resource 可放行而继续。
pub struct ToolGateway {
    grant_store: Arc<dyn PermissionGrantStore>,
    audit_store: Arc<dyn PermissionAuditStore>,
    prompter: Arc<dyn ApprovalPrompter>,
    approval_timeout: ApprovalTimeoutSource,
}

impl ToolGateway {
    pub fn new(
        grant_store: Arc<dyn PermissionGrantStore>,
        audit_store: Arc<dyn PermissionAuditStore>,
        prompter: Arc<dyn ApprovalPrompter>,
        approval_timeout: Duration,
    ) -> Self {
        Self::with_timeout_source(
            grant_store,
            audit_store,
            prompter,
            Arc::new(move || approval_timeout),
        )
    }

    pub fn with_timeout_source(
        grant_store: Arc<dyn PermissionGrantStore>,
        audit_store: Arc<dyn PermissionAuditStore>,
        prompter: Arc<dyn ApprovalPrompter>,
        approval_timeout: ApprovalTimeoutSource,
    ) -> Self {
        Self {
            grant_store,
            audit_store,
            prompter,
            approval_timeout,
        }
    }

    pub async fn evaluate(
        &self,
        tool: &dyn Tool,
        input: &Value,
        context: &ToolUseContext,
        policy: &ResolvedPermissionPolicy,
    ) -> Result<GatewayDecision, String> {
        let tool_name = tool.id();
        let tier = tool.tier();

        // tier 硬上限先于 intent 判定：层级越界的工具即使声明的 intent 全为 Allow
        // 也不得执行（可用性层过滤之外的第二道闸，I38d A7）。
        let tier_resource = format!("tier:{}", tier.as_str());
        if policy.evaluate_tier_ceiling(tier) == PermissionEffect::Deny {
            self.audit(context, tool_name, "*", &tier_resource, Outcome::Denied)
                .await;
            return Ok(GatewayDecision::Denied {
                action: "*".to_string(),
                resource: tier_resource,
            });
        }

        let intents = match tool.permission_intents(input, context) {
            Ok(intents) => intents,
            Err(error) => {
                // 求值失败同样要可审计：detail 只记固定串，不把错误文本（可能夹带入参片段）写进 resource。
                self.audit(
                    context,
                    tool_name,
                    "none",
                    "intent:error",
                    Outcome::IntentError,
                )
                .await;
                return Err(error);
            }
        };
        if intents.is_empty() {
            // 空 intent 是「未经策略求值即执行」，与「策略允许」必须分开记。
            self.audit(context, tool_name, "none", "intent:none", Outcome::Bypassed)
                .await;
            return Ok(GatewayDecision::Allowed);
        }
        let mut pending: Vec<PendingApproval> = Vec::new();
        // 隔离强度取一次：同一次求值内所有 resource 共用同一份探测结论。
        // `None` 表示该工具不是沙箱工具，命令键形状与旧版一致。
        let enforcement = tool.sandbox_enforcement();

        for intent in &intents {
            for resource in &intent.resources {
                let key = ApprovalKey::from_intent(
                    tool_name,
                    intent,
                    resource,
                    input,
                    context,
                    tier,
                    enforcement,
                );
                // 顺序固定（§6.4）：策略先求值，`Deny` 立即拒绝；只有 `Allow` / `Ask`
                // 才查授权缓存。反过来（先查缓存）会让「此后被策略改判 Deny 的键」
                // 因为历史授权继续放行——方向是放宽权限。
                match policy.evaluate(&intent.action, resource) {
                    PermissionEffect::Allow => {
                        if self
                            .grant_hit(key.as_ref(), context, tool_name, &intent.action, resource)
                            .await
                        {
                            continue;
                        }
                        self.audit(
                            context,
                            tool_name,
                            &intent.action,
                            resource,
                            Outcome::Allowed,
                        )
                        .await;
                    }
                    PermissionEffect::Deny => {
                        self.audit(
                            context,
                            tool_name,
                            &intent.action,
                            resource,
                            Outcome::Denied,
                        )
                        .await;
                        return Ok(GatewayDecision::Denied {
                            action: intent.action.clone(),
                            resource: resource.clone(),
                        });
                    }
                    PermissionEffect::Ask => {
                        if self
                            .grant_hit(key.as_ref(), context, tool_name, &intent.action, resource)
                            .await
                        {
                            continue;
                        }
                        pending.push(PendingApproval {
                            key,
                            action: intent.action.clone(),
                            resource: resource.clone(),
                        });
                    }
                }
            }
        }

        if pending.is_empty() {
            return Ok(GatewayDecision::Allowed);
        }

        let request = ApprovalRequest {
            tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
            conversation_id: context.session_id.clone(),
            turn_id: context.turn_id.clone(),
            iteration: context.iteration,
            tool_name: tool_name.to_string(),
            arguments: input.clone(),
            intents: merge_pending(&pending),
            timeout: (self.approval_timeout)(),
            context: build_approval_context(tool, input, context, policy, &pending).await,
        };
        let decision = request_with_timeout(self.prompter.as_ref(), request).await;

        match decision {
            ApprovalWait::Decided(ApprovalDecision::Approve) => {
                for item in &pending {
                    self.audit(
                        context,
                        tool_name,
                        &item.action,
                        &item.resource,
                        Outcome::Approved,
                    )
                    .await;
                }
                Ok(GatewayDecision::Allowed)
            }
            ApprovalWait::Decided(ApprovalDecision::AllowSession) => {
                let conversation_id = context.session_id.as_deref();
                if conversation_id.is_none() {
                    log::warn!(
                        "session grant requested for '{}' without conversation id, not persisting",
                        tool_name
                    );
                }
                for item in &pending {
                    if let (Some(key), Some(conversation_id)) = (item.key.as_ref(), conversation_id)
                    {
                        if let Err(error) = self
                            .grant_store
                            .save_grant(key, GrantScope::Session, Some(conversation_id))
                            .await
                        {
                            log::warn!(
                                "save session grant failed for '{}' on '{}': {}",
                                tool_name,
                                item.resource,
                                error
                            );
                        }
                    }
                    self.audit(
                        context,
                        tool_name,
                        &item.action,
                        &item.resource,
                        Outcome::SessionApproved,
                    )
                    .await;
                }
                Ok(GatewayDecision::Allowed)
            }
            ApprovalWait::Decided(ApprovalDecision::AllowAlways) => {
                for item in &pending {
                    if let Some(key) = item.key.as_ref() {
                        if let Err(error) = self
                            .grant_store
                            .save_grant(key, GrantScope::Global, None)
                            .await
                        {
                            log::warn!(
                                "save grant failed for '{}' on '{}': {}",
                                tool_name,
                                item.resource,
                                error
                            );
                        }
                    }
                    self.audit(
                        context,
                        tool_name,
                        &item.action,
                        &item.resource,
                        Outcome::AlwaysApproved,
                    )
                    .await;
                }
                Ok(GatewayDecision::Allowed)
            }
            ApprovalWait::Decided(ApprovalDecision::Deny { reason }) => {
                for item in &pending {
                    self.audit(
                        context,
                        tool_name,
                        &item.action,
                        &item.resource,
                        Outcome::Rejected,
                    )
                    .await;
                }
                Ok(GatewayDecision::RejectedByUser { reason })
            }
            // 超时同样是拒绝（fail-closed），但审计与下游文案必须能与用户拒绝区分。
            ApprovalWait::TimedOut { timeout } => {
                for item in &pending {
                    self.audit(
                        context,
                        tool_name,
                        &item.action,
                        &item.resource,
                        Outcome::TimedOut,
                    )
                    .await;
                }
                Ok(GatewayDecision::ApprovalTimedOut { timeout })
            }
        }
    }

    /// 授权缓存命中：命中则记 `GrantHit` 并返回 `true`。
    ///
    /// 空键不查缓存：无法归一即无法证明「批准的正是这次操作」。
    /// 命中缓存也要留审计：放行原因（此前授权）与策略 `Allow` 不同。
    async fn grant_hit(
        &self,
        key: Option<&ApprovalKey>,
        context: &ToolUseContext,
        tool: &str,
        action: &str,
        resource: &str,
    ) -> bool {
        let Some(key) = key else {
            return false;
        };
        if !self
            .grant_store
            .is_granted(key, context.session_id.as_deref())
            .await
        {
            return false;
        }
        self.audit(context, tool, action, resource, Outcome::GrantHit)
            .await;
        true
    }

    /// 审计失败只记日志，不改变决策：审计是观测，不是事实源。
    ///
    /// 会话坐标取自 `context`：网关的每条决策都发生在某次工具调用内，
    /// 归属信息必须随行落库，否则事后无法从时间戳反推是哪一轮。
    async fn audit(
        &self,
        context: &ToolUseContext,
        tool: &str,
        action: &str,
        resource: &str,
        outcome: Outcome,
    ) {
        let entry = AuditEntry {
            tool: tool.to_string(),
            action: action.to_string(),
            resource: redact_resource(&redact_secrets(resource)),
            outcome,
            at_millis: now_millis(),
            conversation_id: context.session_id.clone(),
            turn_id: context.turn_id.clone(),
            iteration: context.iteration,
        };
        if let Err(error) = self.audit_store.log_decision(entry).await {
            log::warn!(
                "audit log failed for '{}' '{}' '{}': {}",
                tool,
                action,
                resource,
                error
            );
        }
    }

    /// 供 pipeline 使用的唯一外部写入口：校验阶段发生在网关 `evaluate` 之前，
    /// 但仍必须落到同一个审计存储，禁止在 `ToolPipeline` 里另注入一个 `PermissionAuditStore`。
    ///
    /// `context` 必传：校验失败同样要能归属到具体会话与轮次，否则这类行会成为
    /// 审计里唯一无法定位来源的记录。
    pub async fn audit_external(
        &self,
        context: &ToolUseContext,
        tool: &str,
        action: &str,
        resource: &str,
        outcome: AuditOutcome,
    ) {
        self.audit(context, tool, action, resource, outcome).await;
    }
}
