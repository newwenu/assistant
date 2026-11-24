use std::path::PathBuf;

use assistant_app::adapters::{
    load_project_permissions, mcp_disabled_layer, policy_for_tool_config,
    project_permission_status, save_project_permissions,
};
use assistant_app::protocol::{
    ApprovalLevelDto, ApprovalRuleDto, ApprovalRuleEffectDto, ConversationToolConfigDto,
    ToolTierDto,
};
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
        approval_rules: Vec::new(),
    }
}

fn rule(action: &str, resource: &str, effect: ApprovalRuleEffectDto) -> ApprovalRuleDto {
    ApprovalRuleDto {
        action: action.to_string(),
        resource: resource.to_string(),
        effect,
    }
}

fn config_with_rules(
    tier: ToolTierDto,
    level: ApprovalLevelDto,
    rules: Vec<ApprovalRuleDto>,
) -> ConversationToolConfigDto {
    ConversationToolConfigDto {
        approval_rules: rules,
        ..config(tier, level)
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
fn a_disabled_mcp_server_is_denied_while_others_stay_allowed() {
    assert!(mcp_disabled_layer(&[]).is_none());

    let mut policy =
        policy_for_tool_config(&config(ToolTierDto::External, ApprovalLevelDto::None), None);
    let layer = mcp_disabled_layer(&["srv-a".to_string()]).expect("layer");
    policy = policy.with_constraint_layer(layer);

    // 资源串形如 `mcp:{server_id}:{config_hash}`，禁用层用 `*` 跨过哈希。
    assert_eq!(
        policy.evaluate("mcp", "mcp:srv-a:abc123"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("mcp", "mcp:srv-b:abc123"),
        PermissionEffect::Allow
    );
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

#[test]
fn project_status_reports_unbound_workspace_as_none() {
    assert!(project_permission_status(None).is_none());
}

#[test]
fn project_status_distinguishes_missing_valid_and_invalid() {
    let missing = workspace("status-missing");
    let status = project_permission_status(Some(missing.to_str().unwrap())).expect("status");
    assert!(status.path.ends_with(".assistant/permissions.json"));
    assert!(!status.exists);
    assert!(!status.valid);
    let _ = std::fs::remove_dir_all(missing);

    let valid = workspace("status-valid");
    std::fs::write(
        valid.join(".assistant/permissions.json"),
        r#"{"rules":[{"action":"edit","resource":"**/*.env","effect":"ask"}]}"#,
    )
    .unwrap();
    let status = project_permission_status(Some(valid.to_str().unwrap())).expect("status");
    assert!(status.exists);
    assert!(status.valid);
    let _ = std::fs::remove_dir_all(valid);

    let invalid = workspace("status-invalid");
    std::fs::write(invalid.join(".assistant/permissions.json"), "{ not json").unwrap();
    let status = project_permission_status(Some(invalid.to_str().unwrap())).expect("status");
    assert!(status.exists);
    assert!(!status.valid);
    let _ = std::fs::remove_dir_all(invalid);
}

#[test]
fn project_status_returns_raw_content_for_readonly_preview() {
    let dir = workspace("status-content");
    let text = r#"{"rules":[{"action":"read","resource":"secret/**","effect":"deny"}]}"#;
    std::fs::write(dir.join(".assistant/permissions.json"), text).unwrap();
    let status = project_permission_status(Some(dir.to_str().unwrap())).expect("status");
    assert!(status.exists);
    assert!(status.valid);
    // 原文按设计回传给设置页定位解析错误（I70 §7.5）；正常编辑走结构化规则，不看原文。
    assert_eq!(status.content.as_deref(), Some(text));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn project_status_omits_content_when_file_is_missing() {
    let dir = workspace("status-missing");
    let status = project_permission_status(Some(dir.to_str().unwrap())).expect("status");
    assert!(!status.exists);
    assert!(!status.valid);
    assert_eq!(status.content, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_user_rule_relaxes_the_preset_ask() {
    let config = config_with_rules(
        ToolTierDto::External,
        ApprovalLevelDto::WorkspaceWrite,
        vec![rule("execute", "**", ApprovalRuleEffectDto::Allow)],
    );
    let policy = policy_for_tool_config(&config, None);

    // 用户规则插在预设档之后：把预设的 `Ask` 放宽为 `Allow`（I70 §4.1）。
    assert_eq!(
        policy.evaluate("execute", "ls -la"),
        PermissionEffect::Allow
    );
    // 未写规则的动作仍按预设档：工作区写入放行、敏感文件仍询问。
    assert_eq!(
        policy.evaluate("edit", "/ws/a.txt"),
        PermissionEffect::Allow
    );
    assert_eq!(policy.evaluate("edit", "/ws/.env"), PermissionEffect::Ask);
}

#[test]
fn a_project_deny_overrides_a_user_allow() {
    let dir = workspace("user-vs-project");
    std::fs::write(
        dir.join(".assistant/permissions.json"),
        r#"{"rules":[{"action":"edit","resource":"**/secret.txt","effect":"deny"}]}"#,
    )
    .unwrap();
    let config = config_with_rules(
        ToolTierDto::External,
        ApprovalLevelDto::WorkspaceWrite,
        vec![rule("edit", "**", ApprovalRuleEffectDto::Allow)],
    );
    let policy = policy_for_tool_config(&config, Some(dir.to_str().unwrap()));

    // 项目规则是约束层，逐层取最严：用户 `Allow` 压不过项目 `Deny`。
    assert_eq!(
        policy.evaluate("edit", "/ws/secret.txt"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("edit", "/ws/a.txt"),
        PermissionEffect::Allow
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_user_allow_cannot_lift_the_tier_ceiling() {
    let config = config_with_rules(
        ToolTierDto::Sandbox,
        ApprovalLevelDto::WorkspaceWrite,
        vec![rule("execute", "**", ApprovalRuleEffectDto::Allow)],
    );
    let policy = policy_for_tool_config(&config, None);

    // 规则只改 `(action, resource)` 的结论，不参与层级上限判定（I70 §4.1）。
    assert_eq!(policy.evaluate("execute", "ls"), PermissionEffect::Allow);
    assert_eq!(
        policy.evaluate_tier_ceiling(assistant_tools::ToolTier::Os),
        PermissionEffect::Deny
    );
}

#[test]
fn a3_wildcard_allow_cannot_relax_hard_deny() {
    let config = config_with_rules(
        ToolTierDto::External,
        ApprovalLevelDto::None,
        vec![rule("*", "*", ApprovalRuleEffectDto::Allow)],
    );
    let policy = policy_for_tool_config(&config, None);

    // `("*","*",Allow)` 触碰底线动作，校验拒绝（E4）→ 整层丢弃并升档（fail-closed）。
    assert_eq!(
        policy.evaluate("read", "/etc/shadow"),
        PermissionEffect::Deny
    );
    assert_eq!(policy.evaluate("read", "/ws/a.txt"), PermissionEffect::Ask);
}

#[test]
fn a3_explicit_allow_under_hard_deny_stays_denied() {
    let config = config_with_rules(
        ToolTierDto::External,
        ApprovalLevelDto::None,
        vec![rule("edit", "/etc/**", ApprovalRuleEffectDto::Allow)],
    );
    let policy = policy_for_tool_config(&config, None);

    // 校验放行这条规则，但它排在硬保护之前，硬保护仍在末段生效。
    assert_eq!(
        policy.evaluate("edit", "/etc/passwd"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("edit", "/ws/a.txt"),
        PermissionEffect::Allow
    );
}

#[test]
fn a12_rules_cannot_re_enable_a_disabled_mcp_server() {
    let config = config_with_rules(
        ToolTierDto::External,
        ApprovalLevelDto::None,
        vec![rule("mcp", "*", ApprovalRuleEffectDto::Allow)],
    );
    let mut policy = policy_for_tool_config(&config, None);
    let layer = mcp_disabled_layer(&["srv-a".to_string()]).expect("layer");
    policy = policy.with_constraint_layer(layer);

    // `allow` 触碰底线动作被校验拒绝；即便放行，禁用层仍是更严的约束。
    assert_eq!(
        policy.evaluate("mcp", "mcp:srv-a:abc123"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("mcp", "mcp:srv-b:abc123"),
        PermissionEffect::Ask
    );
}

#[test]
fn a6_over_limit_rules_escalate_and_drop_the_whole_layer() {
    let rules = vec![rule("read", "*", ApprovalRuleEffectDto::Allow); 65];
    let config = config_with_rules(ToolTierDto::External, ApprovalLevelDto::None, rules);
    let policy = policy_for_tool_config(&config, None);

    // 65 条超限（E1）→ 运行时忽略整层规则并升到 `all_os_tools`（fail-closed）。
    assert!(
        policy
            .rules()
            .iter()
            .all(|rule| rule.origin != assistant_tools::RuleOrigin::UserRule),
        "校验失败的规则不得进入策略"
    );
    assert_eq!(policy.evaluate("read", "/ws/a.txt"), PermissionEffect::Ask);
    assert_eq!(policy.evaluate("execute", "ls"), PermissionEffect::Ask);
}

#[test]
fn saving_project_rules_creates_the_file_and_the_directory() {
    // 不预建 `.assistant/`：设置页在缺文件时必须能直接保存成功。
    let dir = std::env::temp_dir().join(format!(
        "assistant-perm-save-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let saved = save_project_permissions(
        dir.to_str().unwrap(),
        &[rule("edit", "**/*.env", ApprovalRuleEffectDto::Ask)],
    )
    .expect("save");
    assert!(saved.exists);
    assert!(saved.valid);
    assert_eq!(saved.rules.len(), 1);
    assert_eq!(saved.rules[0].resource, "**/*.env");

    // 写出的内容仍是用户可直接手工编辑的 `{ "rules": [...] }`，两条写入路径互不排斥。
    let policy = policy_for_tool_config(
        &config(ToolTierDto::External, ApprovalLevelDto::None),
        Some(dir.to_str().unwrap()),
    );
    assert_eq!(policy.evaluate("edit", "/ws/.env"), PermissionEffect::Ask);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_project_rules_leaves_the_file_untouched_when_validation_fails() {
    let dir = workspace("save-invalid");
    let path = dir.join(".assistant/permissions.json");
    let before = r#"{"rules":[{"action":"read","resource":"**","effect":"deny"}]}"#;
    std::fs::write(&path, before).unwrap();

    // 65 条超限（E1）：整体拒绝，磁盘上不能留下半份内容或残缺覆盖。
    let over_limit: Vec<ApprovalRuleDto> = (0..65)
        .map(|_| rule("read", "*", ApprovalRuleEffectDto::Allow))
        .collect();
    let result = save_project_permissions(dir.to_str().unwrap(), &over_limit);
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(dir);
}
