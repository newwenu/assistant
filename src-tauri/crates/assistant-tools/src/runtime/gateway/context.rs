use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::approval_key::ApprovalKey;
use super::stores::redact_resource;
use crate::context::ToolUseContext;
use crate::os::path::{workspace_membership, WorkspaceMembership};
use crate::policy::{PermissionEffect, PermissionRule, ResolvedPermissionPolicy, RuleOrigin};
use crate::r#trait::{ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolTier};
use crate::sandbox::SandboxEnforcement;

/// 审批预演与扩展检测共用的执行预算（I38d §3.2.1）。
///
/// 只在 `Ask` 路径消耗：`Allow` / `Deny` 不会为审批卡片付出任何 IO。
pub const APPROVAL_PREVIEW_BUDGET: Duration = Duration::from_secs(2);

/// 「为什么问我」：命中哪条策略导致 `Ask`。
///
/// 封闭枚举：前端不得窄化，未知分支须能通用降级。P6 产出
/// `DefaultPolicy` / `ApprovalLevel` / `ProjectRule`（层级越界是硬 `Deny`，
/// 不进「为什么问我」，故 `TierCeiling` 暂不产出）；`UserRule` 归用户规则层（I70 §7.1），
/// `Advisor` 归 P10。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalReason {
    DefaultPolicy { rule: String },
    ApprovalLevel { level: String },
    ProjectRule { action: String, resource: String },
    UserRule { action: String, resource: String },
    TierCeiling { tier: ToolTier },
    Advisor { rationale: String },
}

/// 后端检测结论的开放出口（I38d §3.2.1）。
///
/// `kind` + `code` 均为开放集合：前端即使不认识该组合，也能原样渲染 `kind.code`，
/// 不会退化成 `[object Object]`；已知组合由前端按复合键字典本地化（I72 §3.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalExtension {
    pub kind: String,
    pub code: String,
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

pub(crate) struct PendingApproval {
    pub(crate) key: Option<ApprovalKey>,
    pub(crate) action: String,
    pub(crate) resource: String,
}

/// 把待审批项按 action 合并成意图列表，供一次审批请求携带。
pub(crate) fn merge_pending(pending: &[PendingApproval]) -> Vec<PermissionIntent> {
    let mut merged: Vec<PermissionIntent> = Vec::new();
    for item in pending {
        let resource = displayed_resource(item);
        match merged
            .iter_mut()
            .find(|intent| intent.action == item.action)
        {
            Some(intent) => {
                if !intent.resources.contains(&resource) {
                    intent.resources.push(resource);
                }
            }
            None => merged.push(PermissionIntent {
                action: item.action.clone(),
                resources: vec![resource],
            }),
        }
    }
    merged
}

/// 审批卡上展示的资源名。
///
/// 输出域读取的 resource 是会话命名空间目录（应用数据目录下的绝对路径），
/// 只用于审批键覆盖整段命名空间；展示时按既有 `redact_resource` 只留末段，
/// 不把本机绝对路径带进卡片（I52 §5.11.4(5)）。其余动作的资源名保持原样。
fn displayed_resource(item: &PendingApproval) -> String {
    if item.action == "readToolOutput" {
        redact_resource(&item.resource)
    } else {
        item.resource.clone()
    }
}

/// 装配审批卡片的 L0 / L1 / L2 信息（I38d §3.2.1）。
///
/// 只在 `Ask` 路径调用：`preview_for_approval` 的 IO 预算由
/// [`APPROVAL_PREVIEW_BUDGET`] 限制，超时或失败一律降级为 `None`，
/// **不得**改变审批决策，也不得让用户看到一个「0」而不是「未知」。
pub(crate) async fn build_approval_context(
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
        extensions: {
            let mut extensions = workspace_extensions(pending, context);
            extensions.extend(sandbox_extensions(tool));
            extensions
        },
    }
}

/// 沙箱边界扩展（I56 §6.5）：只在 `sandbox_enforcement()` 为 `Some` 的工具上产出。
///
/// 判据取工具自报的强度而不是再探测一次：卡片必须与执行期同源，
/// 否则会出现「卡片说完全隔离、执行按部分隔离放行」。
/// `None` 档不产出任何条目——没有边界就没有可展示的结论，不能把「无隔离」写成一行 `Info`。
/// `full` 档为默认值，仍产出（前端按判据 B 静默丢弃）。
fn sandbox_extensions(tool: &dyn Tool) -> Vec<ApprovalExtension> {
    let Some(enforcement) = tool.sandbox_enforcement() else {
        return Vec::new();
    };
    let (code, severity) = match enforcement {
        SandboxEnforcement::Full => ("full", ApprovalExtensionSeverity::Info),
        SandboxEnforcement::Partial => ("partial", ApprovalExtensionSeverity::Caution),
        SandboxEnforcement::None => return Vec::new(),
    };
    vec![ApprovalExtension {
        kind: "sandbox.enforcement".to_string(),
        code: code.to_string(),
        severity,
    }]
}

/// 路径守卫检测方（I38d §3.2.1 L1 / §4.9）：给出「工作区内/外」结论。
///
/// 结论与执行期边界同源（[`workspace_membership`] 内部复用 `ensure_within_root`），
/// 因此不会出现「审批说在区内、执行说越界」。
///
/// 只对**路径型 action** 判定：`execute`（命令原文）与 `mcp`（`mcp:<server>:<hash>`）
/// 不是路径，解析必然失败并恒判「工作区外」——那是误判而非结论（I72 §5.6.8 G1）。
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
        if !is_path_action(&item.action) {
            continue;
        }
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

    let (code, severity) = if outside == 0 {
        ("inside", ApprovalExtensionSeverity::Info)
    } else if inside == 0 {
        ("outside", ApprovalExtensionSeverity::Caution)
    } else {
        ("partial", ApprovalExtensionSeverity::Caution)
    };

    vec![ApprovalExtension {
        kind: "workspace".to_string(),
        code: code.to_string(),
        severity,
    }]
}

/// 只有这些 action 的 `resource` 是路径；其余（`execute` / `mcp`）不是。
fn is_path_action(action: &str) -> bool {
    matches!(
        action,
        "read" | "readToolOutput" | "edit" | "delete" | "purge"
    )
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
    match rule.origin {
        // 审批级别追加的规则：卡片直接显示「哪个级别要求的审批」，
        // 比投影成 `projectRule` 更能解释「为什么问我」。
        RuleOrigin::ApprovalLevel { level } => ApprovalReason::ApprovalLevel {
            level: level.as_str().to_string(),
        },
        RuleOrigin::UserRule => ApprovalReason::UserRule {
            action: rule.action.clone(),
            resource: rule.resource.clone(),
        },
        RuleOrigin::Config => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{
        policy_from_agent_mode_with_rules, ApprovalLevel, PermissionConstraintLayer,
    };

    fn pending(action: &str, resource: &str) -> PendingApproval {
        PendingApproval {
            key: None,
            action: action.to_string(),
            resource: resource.to_string(),
        }
    }

    fn user_rule(action: &str, resource: &str) -> PermissionRule {
        PermissionRule::new(action, resource, PermissionEffect::Ask)
            .with_origin(RuleOrigin::UserRule)
    }

    #[test]
    fn a_user_rule_ask_is_explained_as_user_rule() {
        let policy = policy_from_agent_mode_with_rules(
            ToolTier::External,
            ApprovalLevel::DangerousOnly,
            &[user_rule("execute", "**")],
        );

        let reasons = ask_reasons(&policy, &[pending("execute", "ls -la")]);
        assert_eq!(
            reasons,
            vec![ApprovalReason::UserRule {
                action: "execute".to_string(),
                resource: "**".to_string(),
            }]
        );
    }

    #[test]
    fn user_and_project_reasons_coexist_without_merging() {
        let policy =
            policy_from_agent_mode_with_rules(
                ToolTier::External,
                ApprovalLevel::DangerousOnly,
                &[user_rule("execute", "**")],
            )
            .with_constraint_layer(PermissionConstraintLayer::new(vec![
                PermissionRule::new("execute", "*", PermissionEffect::Ask),
            ]));

        // 主规则集先于约束层：用户规则不得伪装成项目规则，两条原因并存。
        let reasons = ask_reasons(&policy, &[pending("execute", "ls -la")]);
        assert_eq!(reasons.len(), 2, "主规则集与约束层各保留一条");
        assert_eq!(
            reasons[0],
            ApprovalReason::UserRule {
                action: "execute".to_string(),
                resource: "**".to_string(),
            }
        );
        assert_eq!(
            reasons[1],
            ApprovalReason::ProjectRule {
                action: "execute".to_string(),
                resource: "*".to_string(),
            }
        );
    }

    fn context_with_root(root: &Path) -> ToolUseContext {
        ToolUseContext {
            workspace_root: Some(root.display().to_string()),
            ..Default::default()
        }
    }

    /// G1：`execute` / `mcp` 的 `resource` 不是路径，不得产出 `workspace` 结论。
    #[test]
    fn non_path_actions_do_not_produce_a_workspace_extension() {
        let dir = tempfile::tempdir().unwrap();
        let context = context_with_root(dir.path());

        let extensions = workspace_extensions(
            &[
                pending("execute", "rm -rf build"),
                pending("mcp", "mcp:server:abc123"),
            ],
            &context,
        );
        assert!(
            extensions.is_empty(),
            "command text and mcp ids are not paths (I72 §5.6.8 G1)"
        );
    }

    /// 路径型 action 照常判定：`inside` / `outside` / `partial` 三档与严重度。
    #[test]
    fn path_actions_are_judged_against_the_workspace_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let context = context_with_root(dir.path());
        let inside = dir.path().join("a.txt");
        std::fs::write(&inside, b"x").unwrap();
        let outside = other.path().join("b.txt");

        let inside_only =
            workspace_extensions(&[pending("read", &inside.display().to_string())], &context);
        assert_eq!(inside_only.len(), 1);
        assert_eq!(inside_only[0].kind, "workspace");
        assert_eq!(inside_only[0].code, "inside");
        assert_eq!(inside_only[0].severity, ApprovalExtensionSeverity::Info);

        let mixed = workspace_extensions(
            &[
                pending("edit", &inside.display().to_string()),
                pending("purge", &outside.display().to_string()),
            ],
            &context,
        );
        assert_eq!(mixed.len(), 1);
        assert_eq!(mixed[0].code, "partial");
        assert_eq!(mixed[0].severity, ApprovalExtensionSeverity::Caution);
    }

    /// 未配置工作区：没有边界就没有结论，不得把「无法判定」伪装成「在区内」。
    #[test]
    fn workspace_extension_needs_a_configured_root() {
        let dir = tempfile::tempdir().unwrap();
        let context = ToolUseContext::default();

        let extensions = workspace_extensions(
            &[pending(
                "edit",
                &dir.path().join("a.txt").display().to_string(),
            )],
            &context,
        );
        assert!(extensions.is_empty());
    }

    struct SandboxStub(SandboxEnforcement);

    #[async_trait::async_trait]
    impl Tool for SandboxStub {
        fn id(&self) -> &str {
            "sandbox_stub"
        }

        async fn description(&self) -> Result<String, String> {
            Ok("sandbox stub".to_string())
        }

        fn input_schema(&self) -> Value {
            serde_json::json!({ "type": "object" })
        }

        fn tier(&self) -> ToolTier {
            ToolTier::Os
        }

        fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> {
            Some(self.0)
        }

        async fn execute(
            &self,
            _input: &Value,
            _context: &ToolUseContext,
        ) -> Result<crate::result::ToolResult, String> {
            Ok(crate::result::ToolResult::ok(serde_json::json!({})))
        }
    }

    /// 扩展条目只有两个产出点：`workspace` 与 `sandbox.enforcement`；
    /// `sandbox.root` / `output.scope` 已无产出点（I72 §3.1）。
    #[test]
    fn only_workspace_and_sandbox_enforcement_are_produced() {
        let dir = tempfile::tempdir().unwrap();
        let context = context_with_root(dir.path());
        let inside = dir.path().join("a.txt");
        std::fs::write(&inside, b"x").unwrap();

        let workspace =
            workspace_extensions(&[pending("edit", &inside.display().to_string())], &context);
        let sandbox = sandbox_extensions(&SandboxStub(SandboxEnforcement::Partial));

        let kinds: Vec<&str> = workspace
            .iter()
            .chain(sandbox.iter())
            .map(|extension| extension.kind.as_str())
            .collect();
        assert_eq!(kinds, vec!["workspace", "sandbox.enforcement"]);
        assert_eq!(sandbox[0].code, "partial");
        assert_eq!(sandbox[0].severity, ApprovalExtensionSeverity::Caution);

        // 强度 `None` 不产出任何条目：没有边界就没有可展示的结论。
        assert!(sandbox_extensions(&SandboxStub(SandboxEnforcement::None)).is_empty());
    }
}
