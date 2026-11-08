use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::os::path::{workspace_membership, WorkspaceMembership};
use crate::policy::{PermissionEffect, PermissionRule, ResolvedPermissionPolicy};
use crate::r#trait::{ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolTier};

/// 审批预演与扩展检测共用的执行预算（I38d §3.2.1）。
///
/// 只在 `Ask` 路径消耗：`Allow` / `Deny` 不会为审批卡片付出任何 IO。
pub const APPROVAL_PREVIEW_BUDGET: Duration = Duration::from_secs(2);

/// 审批缓存键的作用域。键必须精确描述「用户批准了什么」，
/// 不允许前缀匹配：批准 `git status` 不等于批准 `git status --short`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ApprovalScope {
    Path {
        canonical: String,
    },
    Command {
        argv: Vec<String>,
        cwd: String,
        sandbox_tier: ToolTier,
    },
    Endpoint {
        host: String,
        method: String,
    },
    Opaque {
        fingerprint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalKey {
    pub tool: String,
    pub scope: ApprovalScope,
}

impl ApprovalKey {
    pub fn new(tool: impl Into<String>, scope: ApprovalScope) -> Self {
        Self {
            tool: tool.into(),
            scope,
        }
    }

    /// 生成单个 resource 的审批键。
    ///
    /// 返回 `None` 表示「空键」：无法归一（路径不可解析、命令分词失败、
    /// 资源为空）时调用方必须直接走审批，不得查询缓存。
    pub fn from_intent(
        tool: &str,
        intent: &PermissionIntent,
        resource: &str,
        input: &Value,
        context: &ToolUseContext,
        tier: ToolTier,
    ) -> Option<Self> {
        let scope = match intent.action.as_str() {
            "read" | "edit" | "write" | "delete" | "move" | "copy" => ApprovalScope::Path {
                canonical: canonical_path_for_key(resource)?,
            },
            "execute" | "run" | "shell" => ApprovalScope::Command {
                argv: split_argv(resource)?,
                cwd: command_cwd(input, context),
                sandbox_tier: tier,
            },
            "network" | "fetch" | "http" => {
                let (host, method) = parse_endpoint(resource, input)?;
                ApprovalScope::Endpoint { host, method }
            }
            _ => ApprovalScope::Opaque {
                fingerprint: opaque_fingerprint(resource)?,
            },
        };
        Some(Self::new(tool, scope))
    }
}

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

/// 授权作用域：与 [`ApprovalKey`] 正交，只回答「这份批准对谁有效」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantScope {
    /// 仅 `conversationId` 相同的会话。
    Session,
    /// 所有会话。
    Global,
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

/// 「为什么问我」：命中哪条策略导致 `Ask`。
///
/// 封闭枚举：前端不得窄化，未知分支须能通用降级。P6 只会产出
/// `DefaultPolicy` / `ProjectRule` / `TierCeiling`；`ApprovalLevel` 与
/// `Advisor` 分别归 P6-C7 与 P10。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalReason {
    DefaultPolicy { rule: String },
    ApprovalLevel { level: String },
    ProjectRule { action: String, resource: String },
    TierCeiling { tier: ToolTier },
    Advisor { rationale: String },
}

/// 后端检测结论的开放出口（I38d §3.2.1）。
///
/// 用 `kind` + `label` + `value` 而非任意 JSON：前端即使不认识 `kind`，
/// 也能按 `label` / `value` 原样渲染，不会退化成 `[object Object]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalExtension {
    pub kind: String,
    pub label: String,
    pub value: String,
    pub severity: ApprovalExtensionSeverity,
}

/// 扩展结论的封闭三档严重度，前端按档位统一着色，不需要理解语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalExtensionSeverity {
    Info,
    Caution,
    Danger,
}

/// 审批卡片的全部辅助信息（I38d §3.2.1 的 L0 / L1 / L2）。
///
/// 新增审批信息只在此扩展，不再给 [`ApprovalRequest`] 加顶层字段。
#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalContext {
    /// L0：工具固有层级。
    pub tier: ToolTier,
    /// L0：行为提示，回答「能不能撤销」。
    pub behavior: BehaviorHints,
    /// L1：命中策略（纯函数推导，无 IO）。
    pub reasons: Vec<ApprovalReason>,
    /// L2：一句话量级预演；`None` 表示「未知」，与「零」可区分。
    pub preview: Option<ApprovalPreview>,
    /// L2：后端检测结论，默认空数组。
    pub extensions: Vec<ApprovalExtension>,
}

impl ApprovalContext {
    /// 无预演、无扩展的兜底构造：审批卡片至少能回答 L0 + L1。
    pub fn minimal(tier: ToolTier, behavior: BehaviorHints, reasons: Vec<ApprovalReason>) -> Self {
        Self {
            tier,
            behavior,
            reasons,
            preview: None,
            extensions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalRequest {
    pub tool_call_id: String,
    /// 事件路由信息：审批事件必须带会话、轮次与迭代号，前端才能按 `turnId` 丢弃迟到事件。
    /// 与 `context` 无关（`context` 只承载审批卡片的信息），因此留在顶层。
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub iteration: Option<u32>,
    pub tool_name: String,
    pub arguments: Value,
    pub intents: Vec<PermissionIntent>,
    pub timeout: Duration,
    /// L0 / L1 / L2 审批辅助信息，仅在 `Ask` 路径装配。
    pub context: ApprovalContext,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub tool: String,
    pub action: String,
    pub resource: String,
    pub outcome: AuditOutcome,
    pub at_millis: i64,
}

#[async_trait]
pub trait PermissionGrantStore: Send + Sync {
    /// `conversation_id` 用于命中会话级授权；`None` 时只命中全局授权。
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool;
    /// 会话级授权必须带 `conversation_id`，缺失时应返回错误（不能静默落成全局）。
    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait PermissionAuditStore: Send + Sync {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String>;
}

struct PendingApproval {
    key: Option<ApprovalKey>,
    action: String,
    resource: String,
}

/// 审批超时的取值来源：每次审批都重新求值，配置改动对下一轮立即生效。
pub type ApprovalTimeoutSource = Arc<dyn Fn() -> Duration + Send + Sync>;

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
        let intents = tool.permission_intents(input, context)?;
        if intents.is_empty() {
            return Ok(GatewayDecision::Allowed);
        }

        let tool_name = tool.id();
        let tier = tool.tier();
        let mut pending: Vec<PendingApproval> = Vec::new();

        for intent in &intents {
            for resource in &intent.resources {
                let key =
                    ApprovalKey::from_intent(tool_name, intent, resource, input, context, tier);
                // 空键不查缓存：无法归一即无法证明「批准的正是这次操作」。
                // 命中缓存也要留审计：放行原因（此前授权）与策略 Allow 不同。
                if let Some(key) = key.as_ref() {
                    if self
                        .grant_store
                        .is_granted(key, context.session_id.as_deref())
                        .await
                    {
                        self.audit(tool_name, &intent.action, resource, AuditOutcome::GrantHit)
                            .await;
                        continue;
                    }
                }
                match policy.evaluate(&intent.action, resource) {
                    PermissionEffect::Allow => {
                        self.audit(tool_name, &intent.action, resource, AuditOutcome::Allowed)
                            .await;
                    }
                    PermissionEffect::Deny => {
                        self.audit(tool_name, &intent.action, resource, AuditOutcome::Denied)
                            .await;
                        return Ok(GatewayDecision::Denied {
                            action: intent.action.clone(),
                            resource: resource.clone(),
                        });
                    }
                    PermissionEffect::Ask => pending.push(PendingApproval {
                        key,
                        action: intent.action.clone(),
                        resource: resource.clone(),
                    }),
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
                        tool_name,
                        &item.action,
                        &item.resource,
                        AuditOutcome::Approved,
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
                        tool_name,
                        &item.action,
                        &item.resource,
                        AuditOutcome::SessionApproved,
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
                        tool_name,
                        &item.action,
                        &item.resource,
                        AuditOutcome::AlwaysApproved,
                    )
                    .await;
                }
                Ok(GatewayDecision::Allowed)
            }
            ApprovalWait::Decided(ApprovalDecision::Deny { reason }) => {
                for item in &pending {
                    self.audit(
                        tool_name,
                        &item.action,
                        &item.resource,
                        AuditOutcome::Rejected,
                    )
                    .await;
                }
                Ok(GatewayDecision::RejectedByUser { reason })
            }
            // 超时同样是拒绝（fail-closed），但审计与下游文案必须能与用户拒绝区分。
            ApprovalWait::TimedOut { timeout } => {
                for item in &pending {
                    self.audit(
                        tool_name,
                        &item.action,
                        &item.resource,
                        AuditOutcome::TimedOut,
                    )
                    .await;
                }
                Ok(GatewayDecision::ApprovalTimedOut { timeout })
            }
        }
    }

    /// 审计失败只记日志，不改变决策：审计是观测，不是事实源。
    async fn audit(&self, tool: &str, action: &str, resource: &str, outcome: AuditOutcome) {
        let entry = AuditEntry {
            tool: tool.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            outcome,
            at_millis: now_millis(),
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
}

/// 把待审批项按 action 合并成意图列表，供一次审批请求携带。
fn merge_pending(pending: &[PendingApproval]) -> Vec<PermissionIntent> {
    let mut merged: Vec<PermissionIntent> = Vec::new();
    for item in pending {
        match merged
            .iter_mut()
            .find(|intent| intent.action == item.action)
        {
            Some(intent) => {
                if !intent.resources.contains(&item.resource) {
                    intent.resources.push(item.resource.clone());
                }
            }
            None => merged.push(PermissionIntent {
                action: item.action.clone(),
                resources: vec![item.resource.clone()],
            }),
        }
    }
    merged
}

/// 装配审批卡片的 L0 / L1 / L2 信息（I38d §3.2.1）。
///
/// 只在 `Ask` 路径调用：`preview_for_approval` 的 IO 预算由
/// [`APPROVAL_PREVIEW_BUDGET`] 限制，超时或失败一律降级为 `None`，
/// **不得**改变审批决策，也不得让用户看到一个「0」而不是「未知」。
async fn build_approval_context(
    tool: &dyn Tool,
    input: &Value,
    context: &ToolUseContext,
    policy: &ResolvedPermissionPolicy,
    pending: &[PendingApproval],
) -> ApprovalContext {
    let reasons = ask_reasons(policy, pending);
    let preview = match tokio::time::timeout(
        APPROVAL_PREVIEW_BUDGET,
        tool.preview_for_approval(input, context),
    )
    .await
    {
        Ok(preview) => preview,
        Err(_) => {
            log::debug!(
                "approval preview timed out for '{}' after {:?}",
                tool.id(),
                APPROVAL_PREVIEW_BUDGET
            );
            None
        }
    };
    ApprovalContext {
        tier: tool.tier(),
        behavior: tool.behavior_hints(),
        reasons,
        preview,
        extensions: workspace_extensions(pending, context),
    }
}

/// 路径守卫检测方（I38d §3.2.1 L1 / §4.9）：给出「工作区内/外」结论。
///
/// 结论与执行期边界同源（[`workspace_membership`] 内部复用 `ensure_within_root`），
/// 因此不会出现「审批说在区内、执行说越界」。
///
/// 未配置工作区时不产出条目：没有边界就没有结论，不得把「无法判定」伪装成「在区内」。
/// 只统计本次待审批的资源（`Allow` 命中的资源不参与，它们不在用户决策范围内）。
fn workspace_extensions(
    pending: &[PendingApproval],
    context: &ToolUseContext,
) -> Vec<ApprovalExtension> {
    if context.workspace_root.is_none() {
        return Vec::new();
    }

    let mut inside = 0usize;
    let mut outside = 0usize;
    let mut seen: HashSet<&str> = HashSet::new();
    for item in pending {
        if !seen.insert(item.resource.as_str()) {
            continue;
        }
        match workspace_membership(Path::new(&item.resource), context) {
            WorkspaceMembership::Inside => inside += 1,
            WorkspaceMembership::Outside => outside += 1,
            WorkspaceMembership::Unconfigured => {}
        }
    }

    if inside == 0 && outside == 0 {
        return Vec::new();
    }

    let (value, severity) = if outside == 0 {
        ("inside the workspace root", ApprovalExtensionSeverity::Info)
    } else if inside == 0 {
        (
            "outside the workspace root",
            ApprovalExtensionSeverity::Caution,
        )
    } else {
        (
            "partly outside the workspace root",
            ApprovalExtensionSeverity::Caution,
        )
    };

    vec![ApprovalExtension {
        kind: "workspace".to_string(),
        label: "Workspace".to_string(),
        value: value.to_string(),
        severity,
    }]
}

/// 把「命中哪条策略」翻译成卡片上的「为什么问我」。
///
/// 只保留效果为 `Ask` 的命中规则：解释 `Allow` 规则与「为什么问我」无关，
/// 出现在卡片上只会误导用户。同一规则在多资源命中时去重。
fn ask_reasons(
    policy: &ResolvedPermissionPolicy,
    pending: &[PendingApproval],
) -> Vec<ApprovalReason> {
    let mut reasons: Vec<ApprovalReason> = Vec::new();
    for item in pending {
        for rule in policy.explain(&item.action, &item.resource) {
            if rule.effect != PermissionEffect::Ask {
                continue;
            }
            let reason = reason_from_rule(rule);
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    }
    reasons
}

fn reason_from_rule(rule: &PermissionRule) -> ApprovalReason {
    if rule.action == "*" && rule.resource == "*" {
        ApprovalReason::DefaultPolicy {
            rule: "* *".to_string(),
        }
    } else {
        ApprovalReason::ProjectRule {
            action: rule.action.clone(),
            resource: rule.resource.clone(),
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScopedGrant {
    scope: GrantScope,
    conversation_id: Option<String>,
    key: ApprovalKey,
}

#[derive(Debug, Default)]
pub struct InMemoryGrantStore {
    grants: Mutex<HashSet<ScopedGrant>>,
}

impl InMemoryGrantStore {
    pub fn len(&self) -> usize {
        self.grants.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl PermissionGrantStore for InMemoryGrantStore {
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool {
        let global = ScopedGrant {
            scope: GrantScope::Global,
            conversation_id: None,
            key: key.clone(),
        };
        let session = conversation_id.map(|id| ScopedGrant {
            scope: GrantScope::Session,
            conversation_id: Some(id.to_string()),
            key: key.clone(),
        });
        self.grants
            .lock()
            .map(|grants| grants.contains(&global) || session.is_some_and(|s| grants.contains(&s)))
            .unwrap_or(false)
    }

    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String> {
        if scope == GrantScope::Session && conversation_id.is_none() {
            return Err("session grant requires conversation id".to_string());
        }
        let grant = ScopedGrant {
            scope,
            conversation_id: conversation_id.map(str::to_string),
            key: key.clone(),
        };
        self.grants
            .lock()
            .map_err(|_| "grant store poisoned".to_string())?
            .insert(grant);
        Ok(())
    }
}

/// 子 Agent 的授权视图：读穿透到父库，写入只落在自己的局部库。
/// 委托产生的批准不得污染父会话的授权集合。
pub struct ChildGrantStore {
    parent: Arc<dyn PermissionGrantStore>,
    local: InMemoryGrantStore,
}

impl ChildGrantStore {
    pub fn new(parent: Arc<dyn PermissionGrantStore>) -> Self {
        Self {
            parent,
            local: InMemoryGrantStore::default(),
        }
    }

    pub fn local(&self) -> &InMemoryGrantStore {
        &self.local
    }
}

#[async_trait]
impl PermissionGrantStore for ChildGrantStore {
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool {
        self.local.is_granted(key, conversation_id).await
            || self.parent.is_granted(key, conversation_id).await
    }

    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String> {
        self.local.save_grant(key, scope, conversation_id).await
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAuditStore {
    entries: Mutex<Vec<AuditEntry>>,
}

impl InMemoryAuditStore {
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl PermissionAuditStore for InMemoryAuditStore {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String> {
        self.entries
            .lock()
            .map_err(|_| "audit store poisoned".to_string())?
            .push(entry);
        Ok(())
    }
}

/// 脚本化审批替身：按脚本依次返回决策，脚本耗尽后回落到 `Deny`（fail-closed）。
pub struct ScriptedApprovalPrompter {
    decisions: Mutex<VecDeque<ApprovalDecision>>,
    requests: Mutex<Vec<ApprovalRequest>>,
    fallback: ApprovalDecision,
    delay: Option<Duration>,
}

impl ScriptedApprovalPrompter {
    pub fn new(decisions: Vec<ApprovalDecision>) -> Self {
        Self {
            decisions: Mutex::new(decisions.into()),
            requests: Mutex::new(Vec::new()),
            fallback: ApprovalDecision::Deny {
                reason: Some("script exhausted".to_string()),
            },
            delay: None,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().map(|r| r.len()).unwrap_or(0)
    }

    pub fn requests(&self) -> Vec<ApprovalRequest> {
        self.requests.lock().map(|r| r.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ApprovalPrompter for ScriptedApprovalPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request);
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.decisions
            .lock()
            .ok()
            .and_then(|mut decisions| decisions.pop_front())
            .unwrap_or_else(|| self.fallback.clone())
    }
}

/// 路径归一：存在的路径交给 `canonicalize`（消解 `..` 与符号链接），
/// 不存在的路径退化为「已存在祖先归一 + 原样尾部」。
///
/// 归一结果必须是合法 UTF-8：审批键是缓存的**匹配键**，用 `to_string_lossy`
/// 会让两条不同的非 UTF-8 路径塌缩成同一键，方向是放宽权限。
/// 无法无损转成字符串时返回 `None`，调用方据此走审批、不查缓存（fail-closed）。
fn canonical_path_for_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    let canonical = dunce::canonicalize(path)
        .or_else(|_| crate::os::path::canonicalize_lenient(path).map_err(std::io::Error::other))
        .ok()?;
    canonical.to_str().map(str::to_string)
}

/// 命令分词：按 shell 风格引号切分为完整 argv。
/// 未闭合引号返回 `None`，避免用半截命令生成可复用键。
fn split_argv(command: &str) -> Option<Vec<String>> {
    let mut argv: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut pending = false;

    for ch in command.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                pending = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                pending = true;
            }
            ch if ch.is_whitespace() && !in_single && !in_double => {
                if pending {
                    argv.push(std::mem::take(&mut current));
                    pending = false;
                }
            }
            ch => {
                current.push(ch);
                pending = true;
            }
        }
    }

    if in_single || in_double {
        return None;
    }
    if pending {
        argv.push(current);
    }
    if argv.is_empty() {
        None
    } else {
        Some(argv)
    }
}

fn command_cwd(input: &Value, context: &ToolUseContext) -> String {
    let raw = input
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| context.workspace_root.clone());
    match raw {
        Some(value) if !value.trim().is_empty() => canonical_path_for_key(&value).unwrap_or(value),
        _ => String::new(),
    }
}

fn parse_endpoint(resource: &str, input: &Value) -> Option<(String, String)> {
    let raw = resource.trim();
    if raw.is_empty() {
        return None;
    }
    let method = input
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let authority = raw
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(raw)
        .split('/')
        .next()
        .unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some((host, method))
    }
}

fn opaque_fingerprint(resource: &str) -> Option<String> {
    let trimmed = resource.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
