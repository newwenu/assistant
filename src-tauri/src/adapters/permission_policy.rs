use std::path::Path;

use assistant_tools::{
    policy_from_agent_mode_with_rules, validate_rules, ApprovalLevel, PermissionConstraintLayer,
    PermissionEffect, PermissionRule, ResolvedPermissionPolicy, ToolTier,
};

use crate::protocol::dto::agent::{ApprovalRuleDto, ConversationToolConfigDto};

/// 项目级权限配置相对工作区根目录的路径（I18 §3.11）。
pub const PROJECT_PERMISSIONS_RELATIVE_PATH: &str = ".assistant/permissions.json";

/// 会话有效工具配置 → 权限策略（I38d §4.6 / I70 §4.1）。
///
/// 层级顺序：预设档规则 → 用户规则（主规则集）→ 硬保护规则（主规则集末段）
/// → tier 上限（约束层）→ 项目级 `permissions.json`（额外约束层，只能收紧）。
/// 每轮现算，改配置后下一轮立即生效。
///
/// 用户规则校验失败时 **fail-closed**（I70 §6.2 / INV-8）：忽略整层规则并把档位
/// 升为 `all_os_tools`，不 panic、不按原档位继续。
pub fn policy_for_tool_config(
    config: &ConversationToolConfigDto,
    workspace_root: Option<&str>,
) -> ResolvedPermissionPolicy {
    let approval_level = ApprovalLevel::from(config.approval_level);
    let (approval_level, user_rules) = resolve_user_rules(approval_level, config);
    let mut policy = policy_from_agent_mode_with_rules(
        ToolTier::from(config.max_tool_tier),
        approval_level,
        &user_rules,
    );
    if let Some(layer) = load_project_permissions(workspace_root) {
        policy = policy.with_constraint_layer(layer);
    }
    policy
}

/// 把会话配置里的 `approvalRules` 投影成内部规则；校验不过则整层丢弃并升档。
///
/// 升档必须与清空规则同时发生：只清规则不升档会让「原本被放宽的调用」悄悄退回
/// 预设档，用户看不到任何提示；升档则让整轮退到最严，属可见的 fail-closed。
fn resolve_user_rules(
    approval_level: ApprovalLevel,
    config: &ConversationToolConfigDto,
) -> (ApprovalLevel, Vec<PermissionRule>) {
    let rules: Vec<PermissionRule> = config
        .approval_rules
        .iter()
        .cloned()
        .map(Into::into)
        .collect();
    if rules.is_empty() {
        return (approval_level, rules);
    }
    if let Err(error) = validate_rules(&rules) {
        log::error!(
            "invalid approval rules for conversation tool config ({error}); \
             ignoring rules and escalating approval level to all_os_tools"
        );
        return (ApprovalLevel::AllOsTools, Vec::new());
    }
    (approval_level, rules)
}

/// 落库前的规则校验（I70 §6.2 / A11）：失败即 `Err`，调用方不得写入半份配置。
///
/// 模式级用户规则与项目级 `permissions.json` 共用这一份实现，避免两处判据漂移。
/// 项目规则写入磁盘时 `origin` 保持 `Config`（解释链据此投影成 `projectRule`），
/// 因此这里**不能**用 `ApprovalRuleDto → PermissionRule` 的 `From`（那会标成 `UserRule`）。
pub fn validate_approval_rules(rules: &[ApprovalRuleDto]) -> Result<(), String> {
    let internal: Vec<PermissionRule> = rules
        .iter()
        .map(|rule| {
            PermissionRule::new(
                rule.action.clone(),
                rule.resource.clone(),
                rule.effect.into(),
            )
        })
        .collect();
    validate_rules(&internal).map_err(|error| {
        let reason = error.to_string();
        let mut args = std::collections::HashMap::new();
        args.insert("reason", reason.as_str());
        assistant_i18n::t_with_args("error.approval_rules_invalid", &args)
    })
}

/// 会话工具配置的规则校验：只取 `approvalRules` 一段。
pub fn validate_tool_config_rules(config: &ConversationToolConfigDto) -> Result<(), String> {
    validate_approval_rules(&config.approval_rules)
}

/// 项目规则文件的解析结果（运行时装配与只读视图共用，避免两处各判一次）。
enum ProjectPermissionsFile {
    Missing,
    Invalid,
    Valid(PermissionConstraintLayer),
}

fn project_permissions_path(root: &str) -> std::path::PathBuf {
    Path::new(root).join(PROJECT_PERMISSIONS_RELATIVE_PATH)
}

/// 读取并解析 `permissions.json`；日志只在这里打，调用方不重复记录。
///
/// 同时返回原始文本（缺失为 `None`）：设置页只读预览需要原文，非法时更要看到
/// 用户自己写下的内容才能定位错误；解析结果仍以 `ProjectPermissionsFile` 为准。
fn read_project_permissions(
    root: &str,
) -> (std::path::PathBuf, ProjectPermissionsFile, Option<String>) {
    let path = project_permissions_path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (path, ProjectPermissionsFile::Missing, None);
        }
        Err(error) => {
            log::error!(
                "failed to read project permissions '{}': {error}; denying all tool calls",
                path.display()
            );
            return (path, ProjectPermissionsFile::Invalid, None);
        }
    };
    match serde_json::from_str::<PermissionConstraintLayer>(&text) {
        Ok(layer) => (path, ProjectPermissionsFile::Valid(layer), Some(text)),
        Err(error) => {
            log::error!(
                "project permissions '{}' is invalid: {error}; denying all tool calls",
                path.display()
            );
            (path, ProjectPermissionsFile::Invalid, Some(text))
        }
    }
}

/// 读取项目级 `permissions.json`。缺失视为「无项目规则」；内容非法时
/// **失败即拒绝**：整层退化为 `("*","*",Deny)`，宁可让工具调用报错，
/// 也不静默丢掉用户写下的安全约束。
pub fn load_project_permissions(workspace_root: Option<&str>) -> Option<PermissionConstraintLayer> {
    let root = workspace_root?;
    match read_project_permissions(root).1 {
        ProjectPermissionsFile::Missing => None,
        ProjectPermissionsFile::Invalid => Some(deny_all_layer()),
        ProjectPermissionsFile::Valid(layer) => Some(layer),
    }
}

/// 项目规则状态（I70 §7.5）：报告路径、解析结果与原始文本，并回传解析出的规则，
/// 供设置页用与模式级相同的编辑器直接编辑后整体写回。
///
/// 原文保留给非法状态：解析失败时只能靠原文定位用户写错的位置。
/// 未绑定工作区时返回 `None`（前端展示「未绑定工作区」，与「文件缺失」可区分）。
pub fn project_permission_status(workspace_root: Option<&str>) -> Option<ProjectPermissionStatus> {
    let root = workspace_root?;
    let (path, file, content) = read_project_permissions(root);
    let rules = match &file {
        ProjectPermissionsFile::Valid(layer) => layer.rules.clone(),
        _ => Vec::new(),
    };
    Some(ProjectPermissionStatus {
        path: path.to_string_lossy().into_owned(),
        exists: !matches!(file, ProjectPermissionsFile::Missing),
        valid: matches!(file, ProjectPermissionsFile::Valid(_)),
        content,
        rules,
    })
}

/// 写入项目级 `permissions.json`（I70 §7.5）：文件不存在时新建，`.assistant/` 一并创建。
///
/// 校验与用户规则共用 [`validate_approval_rules`]：非法规则直接 `Err`，磁盘上不留半份内容。
/// 落盘形态是 `PermissionConstraintLayer`（`{ "rules": [...] }`），与读取侧同一个结构，
/// 因此应用写出的文件仍可被用户手工编辑，两条路径互不排斥。
pub fn save_project_permissions(
    workspace_root: &str,
    rules: &[ApprovalRuleDto],
) -> Result<ProjectPermissionStatus, String> {
    if workspace_root.is_empty() {
        return Err("workspace root is empty".to_string());
    }
    validate_approval_rules(rules)?;

    let internal: Vec<PermissionRule> = rules
        .iter()
        .map(|rule| {
            PermissionRule::new(
                rule.action.clone(),
                rule.resource.clone(),
                rule.effect.into(),
            )
        })
        .collect();
    let path = project_permissions_path(workspace_root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("failed to create '{}': {error}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(&PermissionConstraintLayer::new(internal))
        .map_err(|error| format!("failed to serialize project permissions: {error}"))?;
    text.push('\n');
    std::fs::write(&path, text)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))?;
    project_permission_status(Some(workspace_root))
        .ok_or_else(|| "workspace root is empty".to_string())
}

/// 项目规则视图的数据面：与 `ProjectPermissionStatusDto` 同构，转换在命令层完成。
pub struct ProjectPermissionStatus {
    pub path: String,
    pub exists: bool,
    pub valid: bool,
    /// 文件原文；缺失或不可读为 `None`（前端展示空态，不伪造内容）。
    pub content: Option<String>,
    /// 解析成功的规则；缺失或非法时为空列表。
    pub rules: Vec<PermissionRule>,
}

/// 已禁用 MCP server 的拒绝层（I38e §5）：`mcp` 动作 + `mcp:{server_id}:*` 资源一律 `Deny`。
///
/// 禁用时工具已从 Registry 注销，这一层是「当前 turn 的工具清单里仍残留旧工具」时的兜底：
/// 模型发出迟到调用会被策略拒绝，而不是落到「找不到工具」的内部错误。
/// 资源串中的 `:` 不是路径分隔符，`*` 可以跨过它，因此无需知道 `config_hash`。
pub fn mcp_disabled_layer(disabled_ids: &[String]) -> Option<PermissionConstraintLayer> {
    if disabled_ids.is_empty() {
        return None;
    }
    let rules = disabled_ids
        .iter()
        .map(|id| PermissionRule::new("mcp", format!("mcp:{id}:*"), PermissionEffect::Deny))
        .collect();
    Some(PermissionConstraintLayer::new(rules))
}

fn deny_all_layer() -> PermissionConstraintLayer {
    PermissionConstraintLayer::new(vec![PermissionRule::new("*", "*", PermissionEffect::Deny)])
}
