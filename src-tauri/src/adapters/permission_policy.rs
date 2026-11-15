use std::path::Path;

use assistant_tools::{
    policy_from_agent_mode, ApprovalLevel, PermissionConstraintLayer, PermissionEffect,
    PermissionRule, ResolvedPermissionPolicy, ToolTier,
};

use crate::protocol::dto::agent::ConversationToolConfigDto;

/// 项目级权限配置相对工作区根目录的路径（I18 §3.11）。
pub const PROJECT_PERMISSIONS_RELATIVE_PATH: &str = ".assistant/permissions.json";

/// 会话有效工具配置 → 权限策略（I38d §4.6）。
///
/// 层级顺序：应用默认规则 → 审批级别（主规则集）→ tier 上限（约束层）
/// → 项目级 `permissions.json`（额外约束层，只能收紧）。
/// 每轮现算，改配置后下一轮立即生效。
pub fn policy_for_tool_config(
    config: &ConversationToolConfigDto,
    workspace_root: Option<&str>,
) -> ResolvedPermissionPolicy {
    let mut policy = policy_from_agent_mode(
        ToolTier::from(config.max_tool_tier),
        ApprovalLevel::from(config.approval_level),
    );
    if let Some(layer) = load_project_permissions(workspace_root) {
        policy = policy.with_constraint_layer(layer);
    }
    policy
}

/// 读取项目级 `permissions.json`。缺失视为「无项目规则」；内容非法时
/// **失败即拒绝**：整层退化为 `("*","*",Deny)`，宁可让工具调用报错，
/// 也不静默丢掉用户写下的安全约束。
pub fn load_project_permissions(workspace_root: Option<&str>) -> Option<PermissionConstraintLayer> {
    let root = workspace_root?;
    let path = Path::new(root).join(PROJECT_PERMISSIONS_RELATIVE_PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::error!(
                "failed to read project permissions '{}': {error}; denying all tool calls",
                path.display()
            );
            return Some(deny_all_layer());
        }
    };
    match serde_json::from_str::<PermissionConstraintLayer>(&text) {
        Ok(layer) => Some(layer),
        Err(error) => {
            log::error!(
                "project permissions '{}' is invalid: {error}; denying all tool calls",
                path.display()
            );
            Some(deny_all_layer())
        }
    }
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
