use std::sync::Arc;

use assistant_tools::storage::InMemoryReadReceiptStore;
use assistant_tools::{os, ReadReceiptStore, ToolRegistry, ToolTier};
use serde_json::json;

fn registry() -> ToolRegistry {
    ToolRegistry::new()
}

fn receipts() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

/// 生产注册形态：`fs_link` + 只读组。
fn production_registry() -> ToolRegistry {
    let registry = registry();
    os::register_os_tools(&registry);
    os::register_readonly_tools(&registry, receipts());
    registry
}

#[test]
fn the_readonly_group_registers_the_four_read_tools_and_keeps_fs_link() {
    let registry = production_registry();
    let ids: Vec<String> = {
        let mut ids = registry.tool_ids();
        ids.sort();
        ids
    };
    assert_eq!(
        ids,
        vec!["fs_link", "glob", "grep", "ls", "read"],
        "the read-only group must register exactly read / ls / glob / grep"
    );
}

#[test]
fn the_write_and_dangerous_groups_stay_out_of_the_production_registry() {
    let registry = production_registry();
    for id in ["write", "edit", "mkdir", "move", "delete"] {
        assert!(
            registry.find_for_toolbox(id).is_none(),
            "'{}' must not be reachable before its gate opens",
            id
        );
        assert!(
            registry.find(id).is_none(),
            "'{}' must not be registered",
            id
        );
    }
}

#[tokio::test]
async fn tool_definitions_for_os_tier_only_expose_the_readonly_tools() {
    let registry = production_registry();
    let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
    let ids: Vec<&str> = definitions
        .iter()
        .map(|definition| definition.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["glob", "grep", "ls", "read"],
        "fs_link is TOOLBOX_ONLY and must be filtered out by audience"
    );

    let schemas: Vec<_> = definitions
        .iter()
        .map(|definition| definition.input_schema["type"].clone())
        .collect();
    assert!(
        schemas.iter().all(|schema| schema == &json!("object")),
        "every exposed definition must carry its object schema"
    );
}

#[test]
fn the_write_group_becomes_reachable_when_explicitly_registered() {
    let registry = production_registry();
    os::register_write_tools(&registry, receipts());

    for id in ["write", "edit", "mkdir", "move"] {
        assert!(
            registry.find_for_toolbox(id).is_some(),
            "'{}' must be reachable once the write group is registered",
            id
        );
    }
    assert!(
        registry.find_for_toolbox("delete").is_none(),
        "registering the write group must not pull in the dangerous group"
    );
}

#[test]
fn the_dangerous_group_becomes_reachable_when_explicitly_registered() {
    let registry = production_registry();
    os::register_dangerous_tools(&registry, receipts());

    assert!(
        registry.find_for_toolbox("delete").is_some(),
        "delete must be reachable once the dangerous group is registered"
    );
    for id in ["write", "edit", "mkdir", "move"] {
        assert!(
            registry.find_for_toolbox(id).is_none(),
            "registering the dangerous group must not pull in the write group"
        );
    }
}

#[tokio::test]
async fn the_registered_groups_carry_the_expected_metadata() {
    let registry = production_registry();
    os::register_write_tools(&registry, receipts());
    os::register_dangerous_tools(&registry, receipts());

    let expectations = [
        ("write", ToolTier::Os, false),
        ("edit", ToolTier::Os, false),
        ("mkdir", ToolTier::Os, false),
        ("move", ToolTier::Os, false),
        ("delete", ToolTier::Os, true),
    ];
    for (id, tier, destructive) in expectations {
        let registered = registry
            .find(id)
            .unwrap_or_else(|| panic!("{} missing", id));
        assert_eq!(registered.tool.tier(), tier);
        assert!(registered.tool.audience().agent, "{} is agent facing", id);
        assert!(
            registered.tool.audience().toolbox,
            "{} is toolbox facing",
            id
        );
        assert_eq!(
            registered.tool.behavior_hints().destructive,
            destructive,
            "{}",
            id
        );
        assert!(!registered.tool.behavior_hints().read_only, "{} writes", id);
    }

    let delete = registry.find("delete").unwrap();
    let preview = delete
        .tool
        .preview_for_approval(
            &json!({ "path": "somewhere.txt" }),
            &assistant_tools::ToolUseContext {
                workspace_root: None,
                session_id: None,
                tool_call_id: Some("call-1".to_string()),
            },
        )
        .await;
    assert!(
        preview.is_none(),
        "an unresolvable target must not produce a preview"
    );
}

/// `os_tool_exec` 会把 `action` 注入 `params`；新工具必须忽略这个多余键照常执行。
#[tokio::test]
async fn the_injected_action_key_does_not_break_the_new_tools() {
    let registry = production_registry();
    os::register_write_tools(&registry, receipts());
    os::register_dangerous_tools(&registry, receipts());

    let dir = tempfile::tempdir().unwrap();
    let context =
        assistant_tools::ToolUseContext::for_toolbox(Some(dir.path().display().to_string()));

    let mkdir = registry.find_for_toolbox("mkdir").unwrap();
    let result = assistant_tools::invoke(
        mkdir.tool.as_ref(),
        &json!({ "action": "create", "path": "made" }),
        &context,
        assistant_tools::DEFAULT_TOOL_TIMEOUT,
    )
    .await;
    assert!(result.ok, "{:?}", result.error);
    assert!(dir.path().join("made").is_dir());

    let write = registry.find_for_toolbox("write").unwrap();
    let result = assistant_tools::invoke(
        write.tool.as_ref(),
        &json!({ "action": "write", "path": "made/a.txt", "content": "alpha\n" }),
        &context,
        assistant_tools::DEFAULT_TOOL_TIMEOUT,
    )
    .await;
    assert!(result.ok, "{:?}", result.error);

    let delete = registry.find_for_toolbox("delete").unwrap();
    let result = assistant_tools::invoke(
        delete.tool.as_ref(),
        &json!({ "action": "delete", "path": "made/a.txt" }),
        &context,
        assistant_tools::DEFAULT_TOOL_TIMEOUT,
    )
    .await;
    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("made/a.txt").exists());
}
