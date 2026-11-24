use std::path::Path;

use assistant_tools::os::agent_mkdir::AgentMkdirTool;
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

fn tool() -> AgentMkdirTool {
    AgentMkdirTool::new()
}

#[test]
fn schema_declares_path_and_parents() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["parents"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_mkdir_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    assert_eq!(tool().id(), "mkdir");
    assert_eq!(tool().tier(), ToolTier::Os);
    assert_eq!(tool().audience(), ToolAudience::BOTH);
    assert!(!tool().behavior_hints().read_only);
    assert!(!tool().behavior_hints().destructive);
    assert!(!tool().behavior_hints().open_world);
    assert!(!tool().is_concurrency_safe(None), "mkdir must stay serial");

    let intents = tool()
        .permission_intents(&json!({ "path": "a" }), &ctx)
        .unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].action, "edit");
    assert!(Path::new(&intents[0].resources[0]).is_absolute());
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let cases = [
        json!({}),
        json!({ "path": 42 }),
        json!({ "path": "  " }),
        json!({ "path": "a", "parents": "yes" }),
    ];
    for input in cases {
        assert!(
            !tool().validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool().validate_input(&json!({ "path": "a" }), &ctx),
        ValidationResult::ok()
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let outside = other.path().join("dir");

    let result = tool().validate_input(&json!({ "path": outside.display().to_string() }), &ctx);
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn preview_reports_the_verb_and_the_target_state() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("existing")).unwrap();

    let fresh = tool()
        .preview_for_approval(&json!({ "path": "fresh" }), &ctx)
        .await
        .expect("preview for a missing directory");
    assert_eq!(fresh.summary, "", "结构化齐备时 summary 置空（I72 §5.3）");
    assert_eq!(
        fresh.verb_code.as_deref(),
        Some("mkdir"),
        "mkdir 无判别参数，动词由后端直给（I72 §5.6.8 G2）"
    );
    assert_eq!(fresh.target_kind.as_deref(), Some("directory"));
    assert_eq!(fresh.exists, Some(false));
    assert_eq!(fresh.symlink, Some(false));
    assert_eq!(fresh.entries, None, "目录无规模段");
    assert_eq!(fresh.bytes, None);
    assert!(fresh.shell.is_none());

    let existing = tool()
        .preview_for_approval(&json!({ "path": "existing" }), &ctx)
        .await
        .expect("preview for an existing directory");
    assert_eq!(existing.exists, Some(true));
    assert_eq!(existing.verb_code.as_deref(), Some("mkdir"));
}

#[tokio::test]
async fn creates_a_single_directory() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool()
        .execute(&json!({ "path": "sub" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(dir.path().join("sub").is_dir());
    let data = result.data.clone().unwrap();
    assert_eq!(data["created"], json!(true));
    assert_eq!(data["alreadyExists"], json!(false));
    assert!(result.assistant_text().starts_with("Created directory "));
}

#[tokio::test]
async fn creates_nested_directories_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool()
        .execute(&json!({ "path": "a/b/c", "parents": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(dir.path().join("a/b/c").is_dir());
}

#[tokio::test]
async fn parents_false_fails_when_the_parent_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool()
        .execute(&json!({ "path": "a/b", "parents": false }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("does not exist"));
    assert!(error.contains("parents = true"));
    assert!(!dir.path().join("a").exists());
}

#[tokio::test]
async fn parents_false_succeeds_when_the_parent_exists() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("a")).unwrap();

    let result = tool()
        .execute(&json!({ "path": "a/b", "parents": false }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(dir.path().join("a/b").is_dir());
}

#[tokio::test]
async fn an_existing_directory_is_an_idempotent_success() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("sub")).unwrap();

    let result = tool()
        .execute(&json!({ "path": "sub" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.clone().unwrap();
    assert_eq!(data["created"], json!(false));
    assert_eq!(data["alreadyExists"], json!(true));
    assert!(result.assistant_text().contains("already exists"));
}

#[tokio::test]
async fn a_file_target_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("file.txt"), "x").unwrap();

    let result = tool()
        .execute(&json!({ "path": "file.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("not a directory"));
}

#[tokio::test]
async fn a_parent_that_is_a_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("file.txt"), "x").unwrap();

    let result = tool()
        .execute(&json!({ "path": "file.txt/sub", "parents": false }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    // Unix 的 io 错误文案是 `Not a directory (os error 20)`，Windows 走父目录判定；
    // 断言只看语义，不绑定平台措辞大小写。
    assert!(result
        .error
        .unwrap_or_default()
        .to_lowercase()
        .contains("not a directory"));
}
