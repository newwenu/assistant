use std::path::Path;
use std::sync::Arc;

use assistant_tools::os::agent_move::AgentMoveTool;
use assistant_tools::os::agent_read::file_revision;
use assistant_tools::storage::{InMemoryReadReceiptStore, ReadReceiptKey, ReadReceiptStore};
use assistant_tools::{
    Tool, ToolAudience, ToolErrorKind, ToolTier, ToolUseContext, ValidationResult,
};
use serde_json::json;

fn context(root: &Path, session: Option<&str>) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: session.map(str::to_string),
        tool_call_id: Some("call-1".to_string()),
        ..Default::default()
    }
}

fn store() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

fn tool(receipts: Arc<dyn ReadReceiptStore>) -> AgentMoveTool {
    AgentMoveTool::new(receipts)
}

#[test]
fn schema_declares_source_target_and_overwrite() {
    let schema = tool(store()).input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["source", "target"]));
    assert!(schema["properties"]["source"].is_object());
    assert!(schema["properties"]["target"].is_object());
    assert!(schema["properties"]["overwrite"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_move_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    assert_eq!(tool(store()).id(), "move");
    assert_eq!(tool(store()).tier(), ToolTier::Os);
    assert_eq!(tool(store()).audience(), ToolAudience::BOTH);
    assert!(!tool(store()).behavior_hints().read_only);
    assert!(!tool(store()).behavior_hints().destructive);
    assert!(!tool(store()).behavior_hints().open_world);
    assert!(
        !tool(store()).is_concurrency_safe(None),
        "move must stay serial"
    );

    let intents = tool(store())
        .permission_intents(&json!({ "source": "a.txt", "target": "b.txt" }), &ctx)
        .unwrap();
    assert_eq!(intents.len(), 2, "both paths are affected");
    assert_eq!(intents[0].action, "edit");
    assert_eq!(intents[1].action, "edit");
    assert!(Path::new(&intents[0].resources[0]).is_absolute());
    assert!(Path::new(&intents[1].resources[0]).is_absolute());
    assert!(intents[0].resources[0].ends_with("a.txt"));
    assert!(intents[1].resources[0].ends_with("b.txt"));
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let cases = [
        json!({}),
        json!({ "source": "a.txt" }),
        json!({ "target": "b.txt" }),
        json!({ "source": 42, "target": "b.txt" }),
        json!({ "source": "a.txt", "target": 42 }),
        json!({ "source": "  ", "target": "b.txt" }),
        json!({ "source": "a.txt", "target": "b.txt", "overwrite": "yes" }),
        json!({ "source": "a.txt", "target": "a.txt" }),
    ];
    for input in cases {
        assert!(
            !tool(store()).validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool(store()).validate_input(&json!({ "source": "a.txt", "target": "b.txt" }), &ctx),
        ValidationResult::ok()
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let outside = other.path().join("file.txt");

    let result = tool(store()).validate_input(
        &json!({ "source": outside.display().to_string(), "target": "b.txt" }),
        &ctx,
    );
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));

    let result = tool(store()).validate_input(
        &json!({ "source": "a.txt", "target": outside.display().to_string() }),
        &ctx,
    );
    assert!(!result.ok);
}

#[tokio::test]
async fn moves_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "source": "a.txt", "target": "b.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("a.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "alpha\n"
    );

    let data = result.data.clone().unwrap();
    assert_eq!(data["overwritten"], json!(false));
    assert_eq!(data["isDirectory"], json!(false));
    assert!(data["source"].as_str().unwrap().ends_with("a.txt"));
    assert!(data["target"].as_str().unwrap().ends_with("b.txt"));
    assert!(result.assistant_text().starts_with("Moved "));
}

#[tokio::test]
async fn moves_a_directory() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir_all(dir.path().join("src/nested")).unwrap();
    std::fs::write(dir.path().join("src/nested/a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "source": "src", "target": "dst" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("src").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("dst/nested/a.txt")).unwrap(),
        "alpha\n"
    );
    assert_eq!(result.data.unwrap()["isDirectory"], json!(true));
}

#[tokio::test]
async fn an_existing_target_is_rejected_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "beta\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "source": "a.txt", "target": "b.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("already exists"));
    assert!(error.contains("overwrite = true"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "beta\n"
    );
    assert!(dir.path().join("a.txt").exists());
}

#[tokio::test]
async fn overwrite_replaces_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "beta\n").unwrap();

    let result = tool(store())
        .execute(
            &json!({ "source": "a.txt", "target": "b.txt", "overwrite": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "alpha\n"
    );
    assert!(!dir.path().join("a.txt").exists());
    assert_eq!(result.data.clone().unwrap()["overwritten"], json!(true));
    assert!(result.assistant_text().contains("replacing"));
}

#[tokio::test]
async fn overwrite_an_empty_directory_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(
            &json!({ "source": "a.txt", "target": "empty", "overwrite": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("empty")).unwrap(),
        "alpha\n"
    );
}

#[tokio::test]
async fn overwriting_a_non_empty_directory_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("full")).unwrap();
    std::fs::write(dir.path().join("full/keep.txt"), "keep\n").unwrap();
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(
            &json!({ "source": "a.txt", "target": "full", "overwrite": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("non-empty directory"));
    assert!(error.contains("delete"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("full/keep.txt")).unwrap(),
        "keep\n"
    );
    assert!(dir.path().join("a.txt").exists(), "source must stay intact");
}

#[tokio::test]
async fn a_missing_source_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool(store())
        .execute(&json!({ "source": "nope.txt", "target": "b.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert!(result.error.unwrap_or_default().contains("does not exist"));
}

#[tokio::test]
async fn a_missing_target_parent_is_rejected_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(
            &json!({ "source": "a.txt", "target": "missing/b.txt" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    let error = result.error.unwrap_or_default();
    assert!(error.contains("does not exist"));
    assert!(error.contains("mkdir"));
    assert!(!dir.path().join("missing").exists());
    assert!(dir.path().join("a.txt").exists());
}

#[tokio::test]
async fn the_source_receipt_is_invalidated() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let source = dir.path().join("a.txt");
    std::fs::write(&source, "alpha\n").unwrap();

    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        // 回执键必须与工具解析后的路径一致（macOS 上 `/var` → `/private/var`）。
        path: assistant_tools::os::path::canonicalize_lenient(&source)
            .expect("canonical path")
            .display()
            .to_string(),
    };
    receipts
        .record(
            key.clone(),
            assistant_tools::storage::ReadReceipt::utf8(
                file_revision(&source).unwrap(),
                "call-read",
            ),
        )
        .await;
    assert!(receipts.lookup(&key).await.is_some());

    let result = tool(receipts.clone())
        .execute(&json!({ "source": "a.txt", "target": "b.txt" }), &ctx)
        .await
        .unwrap();
    assert!(result.ok, "{:?}", result.error);

    assert!(
        receipts.lookup(&key).await.is_none(),
        "the moved-away source must lose its receipt"
    );
}
