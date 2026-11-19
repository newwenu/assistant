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

/// 只读批次形态：`fs_link` + 只读组（P5-R1）。
fn readonly_registry() -> ToolRegistry {
    let registry = registry();
    os::register_os_tools(&registry);
    os::register_readonly_tools(&registry, receipts());
    registry
}

/// 修改组形态：`fs_link` + 只读组 + 修改组（P5-R2，P6 启用）。
fn write_registry() -> ToolRegistry {
    let registry = readonly_registry();
    os::register_write_tools(&registry, receipts());
    registry
}

/// 生产注册形态：`fs_link` + 只读组 + 修改组 + 危险组（与 `tool_setup.rs` 一致）。
fn production_registry() -> ToolRegistry {
    let registry = write_registry();
    os::register_dangerous_tools(&registry, receipts());
    registry
}

#[test]
fn the_readonly_group_registers_the_read_tools_and_keeps_fs_link() {
    let registry = readonly_registry();
    let ids: Vec<String> = {
        let mut ids = registry.tool_ids();
        ids.sort();
        ids
    };
    assert_eq!(
        ids,
        vec![
            "fs_link",
            "glob",
            "grep",
            "ls",
            "read",
            "request_tool",
            "super_read"
        ],
        "the read-only group must register exactly read / super_read / ls / glob / grep / \
         request_tool"
    );
}

#[test]
fn the_production_registry_registers_every_group() {
    let registry = production_registry();
    for id in ["write", "edit", "mkdir", "move", "delete"] {
        assert!(
            registry.find_for_toolbox(id).is_some(),
            "'{}' must be reachable in the production registry",
            id
        );
        assert!(
            registry.find(id).is_some(),
            "'{}' must be registered in the production registry",
            id
        );
    }
}

#[tokio::test]
async fn tool_definitions_for_os_tier_expose_every_os_facing_tool() {
    let registry = production_registry();
    let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
    let ids: Vec<&str> = definitions
        .iter()
        .map(|definition| definition.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "delete",
            "edit",
            "glob",
            "grep",
            "ls",
            "mkdir",
            "move",
            "read",
            "request_tool",
            "super_edit",
            "super_read",
            "write"
        ],
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
    let registry = readonly_registry();
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
    let registry = readonly_registry();
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

    let expectations = [
        ("write", ToolTier::Os, false),
        ("edit", ToolTier::Os, false),
        ("super_edit", ToolTier::Os, true),
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
                ..Default::default()
            },
        )
        .await;
    assert!(
        preview.is_none(),
        "an unresolvable target must not produce a preview"
    );
}

/// `os_tool_exec` 会把 `action` 注入 `params`；所有 toolbox 可见工具都必须忽略这个多余键照常执行。
#[tokio::test]
async fn every_toolbox_tool_ignores_the_injected_action_key() {
    let registry = production_registry();

    let dir = tempfile::tempdir().unwrap();
    let context =
        assistant_tools::ToolUseContext::for_toolbox(Some(dir.path().display().to_string()));
    std::fs::write(dir.path().join("seed.txt"), "alpha\n").unwrap();

    let calls = [
        ("ls", json!({ "action": "ls", "path": "." })),
        ("read", json!({ "action": "read", "path": "seed.txt" })),
        (
            "glob",
            json!({ "action": "glob", "pattern": "**/*.txt", "path": "." }),
        ),
        (
            "grep",
            json!({ "action": "grep", "pattern": "alpha", "path": "." }),
        ),
        ("mkdir", json!({ "action": "create", "path": "made" })),
        (
            "write",
            json!({ "action": "write", "path": "made/a.txt", "content": "alpha\n" }),
        ),
        (
            "delete",
            json!({ "action": "delete", "path": "made/a.txt" }),
        ),
    ];

    for (id, input) in calls {
        let registered = registry
            .find_for_toolbox(id)
            .unwrap_or_else(|| panic!("'{}' missing from the toolbox registry", id));
        let result = assistant_tools::invoke(
            registered.tool.as_ref(),
            &input,
            &context,
            assistant_tools::DEFAULT_TOOL_TIMEOUT,
        )
        .await;
        assert!(
            result.ok,
            "'{}' must ignore the injected action key: {:?}",
            id, result.error
        );
    }

    assert!(dir.path().join("made").is_dir());
    assert!(!dir.path().join("made/a.txt").exists());
}

/// T3.3：`ask_user` 明确不声明权限意图，其可见性由网关的 `bypassed` 口径保证。
/// 这条测试防止后人「顺手」补一个 intent 把审计口径改掉。
#[tokio::test]
async fn ask_user_declares_no_permission_intents() {
    struct NeverAsked;

    #[async_trait::async_trait]
    impl assistant_tools::QuestionPrompter for NeverAsked {
        async fn ask(
            &self,
            _request: assistant_tools::QuestionRequest,
        ) -> assistant_tools::QuestionOutcome {
            assistant_tools::QuestionOutcome::Cancelled
        }
    }

    let tool = assistant_tools::os::agent_ask_user::AgentAskUserTool::new(
        Arc::new(NeverAsked),
        Arc::new(|| None),
    );
    let intents = assistant_tools::Tool::permission_intents(
        &tool,
        &json!({ "questions": [{ "question": "q", "header": "h", "options": [] }] }),
        &assistant_tools::ToolUseContext::default(),
    )
    .expect("ask_user must not fail intent evaluation");
    assert!(
        intents.is_empty(),
        "ask_user must not declare any permission intent: {intents:?}"
    );
}
