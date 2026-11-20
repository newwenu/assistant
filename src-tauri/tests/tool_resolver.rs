use std::sync::Arc;

use assistant_app::adapters::resolve_available_tools;
use assistant_app::protocol::{
    ApprovalLevelDto, ConversationToolConfigDto, SendChatMessageArgsDto, ToolTierDto,
};
use assistant_tools::storage::InMemoryReadReceiptStore;
use assistant_tools::{os, ReadReceiptStore, ToolRegistry, ToolTier};

fn receipts() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

/// 全量注册（含危险组），由 `max_tier` / 名单决定最终可见集。
fn full_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    os::register_os_tools(&registry);
    os::register_readonly_tools(&registry, receipts());
    os::register_write_tools(&registry, receipts());
    os::register_dangerous_tools(&registry, receipts());
    registry
}

fn names(tools: &[assistant_providers::types::ToolSpec]) -> Vec<String> {
    tools.iter().map(|tool| tool.name.clone()).collect()
}

#[tokio::test]
async fn an_empty_whitelist_resolves_to_no_tools() {
    let registry = full_registry();
    let tools = resolve_available_tools(&registry, &[], &[], ToolTier::Os).await;
    assert!(
        tools.is_empty(),
        "deny-by-default: an empty whitelist must expose nothing, got {:?}",
        names(&tools)
    );
}

#[tokio::test]
async fn the_whitelist_selects_only_the_named_tools() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "delete".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["delete", "read"],
        "only the named tools may appear, in registry id order"
    );
}

#[tokio::test]
async fn the_blacklist_overrides_the_whitelist() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "write".to_string()];
    let disabled = vec!["write".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &disabled, ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "a tool in both lists must be excluded"
    );
}

#[tokio::test]
async fn unknown_tool_names_are_ignored_silently() {
    let registry = full_registry();
    let enabled = vec![
        "read".to_string(),
        "no-such-tool".to_string(),
        "".to_string(),
    ];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(names(&tools), vec!["read"]);
}

#[tokio::test]
async fn the_max_tier_is_a_hard_cap() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "delete".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Sandbox).await;
    assert!(
        tools.is_empty(),
        "every os tool sits above the sandbox tier, got {:?}",
        names(&tools)
    );
}

#[tokio::test]
async fn toolbox_only_tools_never_reach_the_model() {
    let registry = full_registry();
    let enabled = vec!["fs_link".to_string(), "read".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "fs_link is TOOLBOX_ONLY and must be filtered by audience"
    );
}

#[tokio::test]
async fn the_resolution_order_is_deterministic() {
    let registry = full_registry();
    let enabled = vec![
        "write".to_string(),
        "read".to_string(),
        "glob".to_string(),
        "edit".to_string(),
    ];
    let first = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    let second = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(names(&first), names(&second));
    assert_eq!(names(&first), vec!["edit", "glob", "read", "write"]);
}

/// I58 §3.6 / G17：配置路径是身份过滤，不是关系展开。
///
/// 白名单里显式写了伴随 id 也不能让超级工具进入本轮工具集——`callable` 只有
/// 「模型申请」一个来源。基工具在不在名单里都不改变这一点。
#[tokio::test]
async fn companion_ids_in_the_whitelist_never_get_injected() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "super_read".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "super_read must stay out of the tool set even when explicitly whitelisted"
    );
}

/// 只有伴随 id、没有基工具时什么都不注入。
#[tokio::test]
async fn a_companion_without_its_base_injects_nothing() {
    let registry = full_registry();
    let enabled = vec!["super_read".to_string(), "super_edit".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert!(
        tools.is_empty(),
        "a lone companion id must resolve to nothing, got {:?}",
        names(&tools)
    );
}

/// G21：`disabled_tools` 里的伴随 id 不构成阻断——用户的唯一控制点是基工具。
#[tokio::test]
async fn companion_ids_in_the_blacklist_have_no_effect() {
    let registry = full_registry();
    let enabled = vec!["read".to_string()];
    let disabled = vec!["super_read".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &disabled, ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "a stale companion id in disabledTools must be ignored"
    );
}

/// G17 的反向断言：禁用基工具时伴随项也不会「残留」——它从未进入候选集。
#[tokio::test]
async fn disabling_the_base_tool_leaves_no_orphan_companion() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "super_read".to_string()];
    let disabled = vec!["read".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &disabled, ToolTier::Os).await;
    assert!(
        tools.is_empty(),
        "neither the base nor its companion may survive, got {:?}",
        names(&tools)
    );
}

/// 元工具是普通可勾选条目：不勾就注入不了，模型因而无从申请升级（I58 §3.6）。
#[tokio::test]
async fn the_meta_tool_requires_an_explicit_check() {
    let registry = full_registry();
    let enabled = vec!["read".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "request_tool must not be injected unless the user checks it"
    );
}

#[tokio::test]
async fn the_meta_tool_is_injected_when_checked() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "request_tool".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(names(&tools), vec!["read", "request_tool"]);
}

/// 元工具同样受 `disabled_tools` 否决——它不再享有「不受黑名单影响」的特权。
#[tokio::test]
async fn the_meta_tool_can_be_disabled() {
    let registry = full_registry();
    let enabled = vec!["read".to_string(), "request_tool".to_string()];
    let disabled = vec!["request_tool".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &disabled, ToolTier::Os).await;
    assert_eq!(
        names(&tools),
        vec!["read"],
        "a disabled meta tool must be excluded like any other entry"
    );
}

/// 只勾元工具、不勾任何基工具：它照常注入，但清单里没有可申请对象。
#[tokio::test]
async fn a_lone_meta_tool_still_resolves() {
    let registry = full_registry();
    let enabled = vec!["request_tool".to_string()];
    let tools = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(names(&tools), vec!["request_tool"]);
}

/// A18：`shell` 的可见性 = 注册门禁 ∩ 勾选 ∩ 层级上限。
///
/// 三段串联才是用户看到的事实：探测不可用 → 不注册（`register_shell_tools` 直接返回）；
/// 注册了但没勾 → 白名单落空；勾了但 `max_tool_tier < os` → 被层级硬上限过滤。
#[tokio::test]
async fn shell_visibility_needs_registration_checkbox_and_tier() {
    use assistant_tools::os::register_shell_tools;
    use assistant_tools::sandbox::{detect, SandboxEnforcement, SandboxHostPaths};
    use assistant_tools::storage::FileToolResultStorage;

    let storage = tempfile::tempdir().expect("storage tempdir");
    let backend: Arc<dyn assistant_tools::ToolResultStorage> =
        Arc::new(FileToolResultStorage::new(storage.path()));
    let probe = detect(&SandboxHostPaths::default());
    assert_ne!(
        probe.enforcement,
        SandboxEnforcement::None,
        "sandbox unavailable on this machine: {:?}",
        probe.detail
    );
    let probe: &'static assistant_tools::sandbox::SandboxProbe = Box::leak(Box::new(probe));

    let registry = ToolRegistry::new();
    register_shell_tools(&registry, backend, probe);
    let enabled = vec!["shell".to_string()];

    let checked = resolve_available_tools(&registry, &enabled, &[], ToolTier::Os).await;
    assert_eq!(
        names(&checked),
        vec!["shell"],
        "checked shell above its tier must reach the model"
    );

    let below_tier = resolve_available_tools(&registry, &enabled, &[], ToolTier::Sandbox).await;
    assert!(
        below_tier.is_empty(),
        "an os tool must stay hidden below its tier, got {:?}",
        names(&below_tier)
    );

    let unchecked = resolve_available_tools(&registry, &[], &[], ToolTier::Os).await;
    assert!(
        unchecked.is_empty(),
        "deny-by-default: an unchecked shell must not be injected, got {:?}",
        names(&unchecked)
    );
}

#[test]
fn the_tool_config_dto_round_trips_through_json() {
    let config = ConversationToolConfigDto {
        max_tool_tier: ToolTierDto::Os,
        approval_level: ApprovalLevelDto::AllOsTools,
        enabled_tools: vec!["read".to_string()],
        disabled_tools: vec!["delete".to_string()],
    };
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["maxToolTier"], serde_json::json!("os"));
    assert_eq!(json["approvalLevel"], serde_json::json!("all_os_tools"));
    assert_eq!(json["enabledTools"], serde_json::json!(["read"]));
    assert_eq!(json["disabledTools"], serde_json::json!(["delete"]));

    let parsed: ConversationToolConfigDto = serde_json::from_value(json).unwrap();
    assert_eq!(parsed, config);
}

#[test]
fn the_default_tool_config_is_fail_closed() {
    let config = ConversationToolConfigDto::default();
    assert_eq!(config.max_tool_tier, ToolTierDto::Sandbox);
    assert_eq!(config.approval_level, ApprovalLevelDto::None);
    assert!(config.enabled_tools.is_empty());
    assert!(config.disabled_tools.is_empty());
}

#[test]
fn send_args_accept_a_missing_tool_config() {
    let args: SendChatMessageArgsDto = serde_json::from_value(serde_json::json!({
        "content": "hello",
        "providerId": "p-1",
        "modelId": "m-1",
    }))
    .expect("toolConfig is optional");
    assert!(args.tool_config.is_none());

    let args: SendChatMessageArgsDto = serde_json::from_value(serde_json::json!({
        "content": "hello",
        "providerId": "p-1",
        "modelId": "m-1",
        "toolConfig": {
            "maxToolTier": "os",
            "approvalLevel": "dangerous_only",
            "enabledTools": ["read"],
            "disabledTools": [],
        },
    }))
    .expect("a full toolConfig parses");
    let config = args.tool_config.expect("toolConfig present");
    assert_eq!(config.approval_level, ApprovalLevelDto::DangerousOnly);
    assert_eq!(config.enabled_tools, vec!["read".to_string()]);
}
