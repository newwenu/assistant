use assistant_tools::policy::{
    default_global_rules, CeilingValidationError, PermissionConstraintLayer, PermissionEffect,
    PermissionRule, PermissionRuntimeCeiling, ResolvedPermissionPolicy,
};

#[test]
fn most_restrictive_covers_nine_combinations() {
    use PermissionEffect::{Allow, Ask, Deny};

    let cases = [
        (Allow, Allow, Allow),
        (Allow, Ask, Ask),
        (Allow, Deny, Deny),
        (Ask, Allow, Ask),
        (Ask, Ask, Ask),
        (Ask, Deny, Deny),
        (Deny, Allow, Deny),
        (Deny, Ask, Deny),
        (Deny, Deny, Deny),
    ];

    for (left, right, expected) in cases {
        assert_eq!(
            left.most_restrictive(right),
            expected,
            "{:?} + {:?}",
            left,
            right
        );
        assert_eq!(
            right.most_restrictive(left),
            expected,
            "commutative: {:?} + {:?}",
            right,
            left
        );
    }
}

#[test]
fn last_match_wins_within_a_single_rule_set() {
    let policy = ResolvedPermissionPolicy::new(
        vec![
            PermissionRule::new("*", "*", PermissionEffect::Ask),
            PermissionRule::new("read", "*", PermissionEffect::Allow),
        ],
        Vec::new(),
    );

    assert_eq!(
        policy.evaluate("read", "src/main.rs"),
        PermissionEffect::Allow
    );
    assert_eq!(
        policy.evaluate("edit", "src/main.rs"),
        PermissionEffect::Ask
    );
}

#[test]
fn specific_rule_overrides_broad_rule_in_same_layer() {
    let policy = ResolvedPermissionPolicy::new(
        vec![
            PermissionRule::new("read", "*", PermissionEffect::Allow),
            PermissionRule::new("read", "/etc/shadow", PermissionEffect::Deny),
        ],
        Vec::new(),
    );

    assert_eq!(
        policy.evaluate("read", "/etc/shadow"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("read", "/etc/hosts"),
        PermissionEffect::Allow
    );
}

#[test]
fn constraint_layer_can_only_tighten() {
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        vec![PermissionConstraintLayer::new(vec![PermissionRule::new(
            "read",
            "**",
            PermissionEffect::Deny,
        )])],
    );

    assert_eq!(policy.evaluate("read", "a/b"), PermissionEffect::Deny);
    assert_eq!(policy.evaluate("edit", "a/b"), PermissionEffect::Allow);
}

#[test]
fn constraint_layer_cannot_relax_deny() {
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new(
            "edit",
            "C:/Windows/**",
            PermissionEffect::Deny,
        )],
        vec![PermissionConstraintLayer::new(vec![PermissionRule::new(
            "*",
            "*",
            PermissionEffect::Allow,
        )])],
    );

    assert_eq!(
        policy.evaluate("edit", "C:/Windows/System32"),
        PermissionEffect::Deny
    );
}

#[test]
fn runtime_ceiling_rejects_allow_rules() {
    let error = PermissionRuntimeCeiling::try_new(vec![
        PermissionRule::new("read", "*", PermissionEffect::Deny),
        PermissionRule::new("edit", "*", PermissionEffect::Allow),
    ])
    .unwrap_err();

    assert_eq!(
        error,
        CeilingValidationError::AllowRule {
            action: "edit".to_string(),
            resource: "*".to_string(),
        }
    );
    assert!(error.to_string().contains("must not relax"));
}

#[test]
fn runtime_ceiling_accepts_deny_and_ask_rules() {
    let ceiling = PermissionRuntimeCeiling::try_new(vec![
        PermissionRule::new("edit", "C:/Windows/**", PermissionEffect::Deny),
        PermissionRule::new("network", "*", PermissionEffect::Ask),
    ])
    .unwrap();

    assert_eq!(ceiling.rules().len(), 2);
}

#[test]
fn runtime_ceiling_layer_tightens_a_permissive_policy() {
    let ceiling = PermissionRuntimeCeiling::try_new(vec![PermissionRule::new(
        "network",
        "*",
        PermissionEffect::Deny,
    )])
    .unwrap();
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        Vec::new(),
    )
    .with_constraint_layer(ceiling.constraint_layer());

    assert_eq!(
        policy.evaluate("network", "example.com"),
        PermissionEffect::Deny
    );
    assert_eq!(policy.evaluate("read", "a/b"), PermissionEffect::Allow);
}

#[test]
fn empty_rule_set_defaults_to_allow_within_a_layer() {
    let policy = ResolvedPermissionPolicy::new(Vec::new(), Vec::new());

    assert_eq!(
        policy.evaluate("edit", "C:/Windows/System32"),
        PermissionEffect::Allow
    );
}

#[test]
fn default_global_rules_require_approval_by_default() {
    let policy = ResolvedPermissionPolicy::default_global();

    assert_eq!(policy.evaluate("execute", "git"), PermissionEffect::Ask);
    assert_eq!(
        policy.evaluate("network", "example.com"),
        PermissionEffect::Ask
    );
    assert_eq!(policy.evaluate("unknown", "x"), PermissionEffect::Ask);
}

#[test]
fn default_global_rules_allow_reads() {
    let policy = ResolvedPermissionPolicy::default_global();

    assert_eq!(
        policy.evaluate("read", "src/main.rs"),
        PermissionEffect::Allow
    );
    assert_eq!(
        policy.evaluate("read", "C:/Users/me/a.txt"),
        PermissionEffect::Allow
    );
}

#[test]
fn default_global_rules_deny_sensitive_reads() {
    let policy = ResolvedPermissionPolicy::default_global();

    assert_eq!(
        policy.evaluate("read", "/etc/shadow"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("read", "/etc/ssh/sshd_config"),
        PermissionEffect::Deny
    );
}

#[test]
fn default_global_rules_deny_system_edits() {
    let policy = ResolvedPermissionPolicy::default_global();

    assert_eq!(
        policy.evaluate("edit", "/etc/hosts"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("edit", "C:/Windows/System32/drivers/etc/hosts"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("edit", "C:/Program Files/app/config.ini"),
        PermissionEffect::Deny
    );
}

#[test]
fn backslashes_match_forward_slash_patterns() {
    let policy = ResolvedPermissionPolicy::default_global();

    assert_eq!(
        policy.evaluate("edit", "C:\\Windows\\System32\\drivers"),
        PermissionEffect::Deny
    );
}

#[test]
fn single_star_does_not_cross_separators() {
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new(
            "read",
            "/etc/*",
            PermissionEffect::Deny,
        )],
        Vec::new(),
    );

    assert_eq!(
        policy.evaluate("read", "/etc/hosts"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("read", "/etc/ssh/sshd_config"),
        PermissionEffect::Allow
    );
}

#[test]
fn double_star_crosses_separators() {
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new(
            "read",
            "/etc/**",
            PermissionEffect::Deny,
        )],
        Vec::new(),
    );

    assert_eq!(
        policy.evaluate("read", "/etc/hosts"),
        PermissionEffect::Deny
    );
    assert_eq!(
        policy.evaluate("read", "/etc/ssh/sshd_config"),
        PermissionEffect::Deny
    );
}

#[test]
fn action_star_matches_every_action() {
    let rule = PermissionRule::new("*", "*", PermissionEffect::Ask);

    assert!(rule.matches("read", "a"));
    assert!(rule.matches("execute", "git"));
    assert!(rule.matches("delete", "b"));
}

#[test]
fn tier_resources_match_literally() {
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new(
            "execute",
            "tier:os",
            PermissionEffect::Ask,
        )],
        Vec::new(),
    );

    assert_eq!(policy.evaluate("execute", "tier:os"), PermissionEffect::Ask);
    assert_eq!(
        policy.evaluate("execute", "tier:external"),
        PermissionEffect::Allow
    );
}

#[test]
fn effect_serializes_camel_case() {
    let value = serde_json::to_value(PermissionEffect::Ask).unwrap();
    assert_eq!(value, serde_json::json!("ask"));
}

#[test]
fn rule_serializes_camel_case() {
    let rule = PermissionRule::new("read", "*", PermissionEffect::Allow);
    let value = serde_json::to_value(&rule).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "action": "read", "resource": "*", "effect": "allow" })
    );
}

#[test]
fn default_global_rules_are_stable() {
    let rules = default_global_rules();

    assert_eq!(rules.len(), 7);
    assert_eq!(rules[0].effect, PermissionEffect::Ask);
    assert_eq!(rules[0].action, "*");
    assert_eq!(rules[0].resource, "*");
}
