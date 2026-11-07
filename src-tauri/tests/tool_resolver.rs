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
        "registry order is by id"
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
    assert_eq!(config.max_tool_tier, ToolTierDto::Os);
    assert_eq!(config.approval_level, ApprovalLevelDto::DangerousOnly);
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
