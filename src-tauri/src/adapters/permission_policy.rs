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

fn deny_all_layer() -> PermissionConstraintLayer {
    PermissionConstraintLayer::new(vec![PermissionRule::new("*", "*", PermissionEffect::Deny)])
}
