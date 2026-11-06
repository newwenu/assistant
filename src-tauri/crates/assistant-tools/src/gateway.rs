use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::policy::{PermissionEffect, ResolvedPermissionPolicy};
use crate::r#trait::{PermissionIntent, Tool, ToolTier};

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
    Denied { action: String, resource: String },
    RejectedByUser { reason: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Deny { reason: Option<String> },
    AlwaysApprove,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalRequest {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub intents: Vec<PermissionIntent>,
    pub timeout: Duration,
}

#[async_trait]
pub trait ApprovalPrompter: Send + Sync {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision;
}

/// 审批等待的唯一入口：超时归一为 `Deny`，绝不默认放行，也不无限挂起。
pub async fn request_with_timeout(
    prompter: &dyn ApprovalPrompter,
    request: ApprovalRequest,
) -> ApprovalDecision {
    let timeout = request.timeout;
    match tokio::time::timeout(timeout, prompter.request(request)).await {
        Ok(decision) => decision,
        Err(_) => ApprovalDecision::Deny {
            reason: Some("approval timed out".to_string()),
        },
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
    AlwaysApproved,
    Rejected,
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
    async fn is_granted(&self, key: &ApprovalKey) -> bool;
    async fn save_grant(&self, key: &ApprovalKey) -> Result<(), String>;
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

/// 工具调用网关：把「工具声明的权限意图」转换成 `GatewayDecision`。
///
/// 决策顺序固定（规格 P2-C4）：intents → 逐 resource 查 grant → policy 判定
/// → 收集 Ask → 合并一次审批 → `AlwaysApprove` 落 grant → 审计。
/// 任一 `Deny` 立即返回，绝不因为其它 resource 可放行而继续。
pub struct ToolGateway {
    grant_store: Arc<dyn PermissionGrantStore>,
    audit_store: Arc<dyn PermissionAuditStore>,
    prompter: Arc<dyn ApprovalPrompter>,
    approval_timeout: Duration,
}

impl ToolGateway {
    pub fn new(
        grant_store: Arc<dyn PermissionGrantStore>,
        audit_store: Arc<dyn PermissionAuditStore>,
        prompter: Arc<dyn ApprovalPrompter>,
        approval_timeout: Duration,
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
                    if self.grant_store.is_granted(key).await {
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
            tool_name: tool_name.to_string(),
            arguments: input.clone(),
            intents: merge_pending(&pending),
            timeout: self.approval_timeout,
        };
        let decision = request_with_timeout(self.prompter.as_ref(), request).await;

        match decision {
            ApprovalDecision::Approve => {
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
            ApprovalDecision::AlwaysApprove => {
                for item in &pending {
                    if let Some(key) = item.key.as_ref() {
                        if let Err(error) = self.grant_store.save_grant(key).await {
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
            ApprovalDecision::Deny { reason } => {
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

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Default)]
pub struct InMemoryGrantStore {
    grants: Mutex<HashSet<ApprovalKey>>,
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
    async fn is_granted(&self, key: &ApprovalKey) -> bool {
        self.grants
            .lock()
            .map(|grants| grants.contains(key))
            .unwrap_or(false)
    }

    async fn save_grant(&self, key: &ApprovalKey) -> Result<(), String> {
        self.grants
            .lock()
            .map_err(|_| "grant store poisoned".to_string())?
            .insert(key.clone());
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
    async fn is_granted(&self, key: &ApprovalKey) -> bool {
        self.local.is_granted(key).await || self.parent.is_granted(key).await
    }

    async fn save_grant(&self, key: &ApprovalKey) -> Result<(), String> {
        self.local.save_grant(key).await
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
