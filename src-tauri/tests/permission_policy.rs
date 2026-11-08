use std::path::PathBuf;

use assistant_app::adapters::{load_project_permissions, policy_for_tool_config};
use assistant_app::protocol::{ApprovalLevelDto, ConversationToolConfigDto, ToolTierDto};
use assistant_tools::PermissionEffect;

fn workspace(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("assistant-perm-{name}-{stamp}"));
    std::fs::create_dir_all(dir.join(".assistant")).unwrap();
    dir
}

fn config(tier: ToolTierDto, level: ApprovalLevelDto) -> ConversationToolConfigDto {
    ConversationToolConfigDto {
        max_tool_tier: tier,
        approval_level: level,
        enabled_tools: vec!["agent_read".to_string()],
        disabled_tools: Vec::new(),
    }
}

#[test]
fn a_missing_project_file_adds_no_constraint_layer() {
    let dir = workspace("missing");
    assert!(load_project_permissions(Some(dir.to_str().unwrap())).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_unset_workspace_adds_no_constraint_layer() {
    assert!(load_project_permissions(None).is_none());
}

#[test]
fn a_project_rule_tightens_the_agent_level_policy() {
    let dir = workspace("tighten");
    std::fs::write(
        dir.join(".assistant/permissions.json"),
        r#"{"rules":[{"action":"edit","resource":"**/*.env","effect":"ask"}]}"#,
    )
    .unwrap();

    // `none` 级别默认自动执行；项目规则把它收紧为需审批。
    let policy = policy_for_tool_config(
        &config(ToolTierDto::External, ApprovalLevelDto::None),
        Some(dir.to_str().unwrap()),
    );
    assert_eq!(policy.evaluate("edit", "/ws/.env"), PermissionEffect::Ask);
    assert_eq!(
        policy.evaluate("edit", "/ws/a.txt"),
        PermissionEffect::Allow
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_project_rule_cannot_relax_the_agent_level_policy() {
    let dir = workspace("relax");
    std::fs::write(
        dir.join(".assistant/permissions.json"),
        r#"{"rules":[{"action":"read","resource":"**","effect":"allow"}]}"#,
    )
    .unwrap();

    // `all_os_tools` 要求读取审批；项目层写 `allow` 只能收紧、不能放宽。
    let policy = policy_for_tool_config(
        &config(ToolTierDto::External, ApprovalLevelDto::AllOsTools),
        Some(dir.to_str().unwrap()),
    );
    assert_eq!(policy.evaluate("read", "/ws/a.txt"), PermissionEffect::Ask);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_invalid_project_file_denies_everything() {
    let dir = workspace("invalid");
    std::fs::write(dir.join(".assistant/permissions.json"), "{ not json").unwrap();

    let policy = policy_for_tool_config(
        &config(ToolTierDto::External, ApprovalLevelDto::None),
        Some(dir.to_str().unwrap()),
    );
    assert_eq!(policy.evaluate("read", "/ws/a.txt"), PermissionEffect::Deny);
    assert_eq!(policy.evaluate("edit", "/ws/a.txt"), PermissionEffect::Deny);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_tier_ceiling_comes_from_the_config_tier() {
    let policy =
        policy_for_tool_config(&config(ToolTierDto::Sandbox, ApprovalLevelDto::None), None);
    assert_eq!(
        policy.evaluate_tier_ceiling(assistant_tools::ToolTier::Os),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate_tier_ceiling(assistant_tools::ToolTier::Sandbox),
        PermissionEffect::Allow
    );
}
