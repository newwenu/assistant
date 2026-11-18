use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::approval_key::ApprovalKey;
use super::stores::redact_resource;
use crate::context::ToolUseContext;
use crate::os::path::{
    canonicalize_lenient, ensure_within_root, workspace_membership, WorkspaceMembership,
};
use crate::policy::{PermissionEffect, PermissionRule, ResolvedPermissionPolicy, RuleOrigin};
use crate::r#trait::{ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolTier};

/// 审批预演与扩展检测共用的执行预算（I38d §3.2.1）。
///
/// 只在 `Ask` 路径消耗：`Allow` / `Deny` 不会为审批卡片付出任何 IO。
pub const APPROVAL_PREVIEW_BUDGET: Duration = Duration::from_secs(2);

/// 「为什么问我」：命中哪条策略导致 `Ask`。
///
/// 封闭枚举：前端不得窄化，未知分支须能通用降级。P6 产出
/// `DefaultPolicy` / `ApprovalLevel` / `ProjectRule`（层级越界是硬 `Deny`，
/// 不进「为什么问我」，故 `TierCeiling` 暂不产出）；`Advisor` 归 P10。
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
            extensions.extend(output_extensions(pending, context));
            extensions
        },
    }
}

/// 输出域检测方（I52 §5.11.4(5)）：本次待审批资源是否落在本会话工具输出目录内。
///
/// 与 [`workspace_extensions`] 完全同构：只凭 `pending.resource` 判定，零 IO。
/// 任何需要 `stat` 的量级信息（文件大小、行窗口）留在 `preview_for_approval`，
/// 不得把 IO 预算与超时降级扩散到扩展构造路径。
fn output_extensions(
    pending: &[PendingApproval],
    context: &ToolUseContext,
) -> Vec<ApprovalExtension> {
    let Some(root) = context.tool_output_root.as_deref() else {
        return Vec::new();
    };
    // 与执行期边界校验同源：`ensure_within_root` 内部对根做宽松规范化，
    // 因此这里不新造第二套路径比较（否则审批卡片与实际判定会分叉）。
    let inside = pending.iter().any(|item| {
        canonicalize_lenient(Path::new(&item.resource))
            .map(|path| ensure_within_root(&path, root).is_ok())
            .unwrap_or(false)
    });
    if !inside {
        return Vec::new();
    }
    // 文案与既有 `workspace` 扩展保持同一种语言（英文），同一张卡片不混排中英。
    vec![ApprovalExtension {
        kind: "output.scope".to_string(),
        label: "Output".to_string(),
        value: "this session's tool output directory (read-only)".to_string(),
        severity: ApprovalExtensionSeverity::Info,
    }]
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
    match rule.origin {
        // 审批级别追加的规则：卡片直接显示「哪个级别要求的审批」，
        // 比投影成 `projectRule` 更能解释「为什么问我」。
        RuleOrigin::ApprovalLevel { level } => ApprovalReason::ApprovalLevel {
            level: level.as_str().to_string(),
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
