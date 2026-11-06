use std::path::Path;

use assistant_tools::os::agent_list::{AgentListTool, MAX_DEPTH, MAX_ENTRIES_PER_DIR};
use assistant_tools::{Tool, ToolAudience, ToolErrorKind, ToolTier, ToolUseContext};
use serde_json::json;

fn context(root: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: None,
        tool_call_id: Some("call-1".to_string()),
    }
}

fn tool() -> AgentListTool {
    AgentListTool::new()
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dir");
    }
    std::fs::write(path, "x").expect("write fixture");
}

#[test]
fn schema_declares_path_depth_and_all() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["depth"].is_object());
    assert!(schema["properties"]["all"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_readonly_contract() {
    let tool = tool();
    assert_eq!(tool.id(), "ls");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(tool.is_concurrency_safe(None));
    assert_eq!(
        tool.permission_intents(&json!({ "path": "." }), &ToolUseContext::default()),
        Ok(vec![])
    );
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let tool = tool();

    let cases = [
        json!({}),
        json!({ "path": 42 }),
        json!({ "path": "  " }),
        json!({ "path": ".", "depth": 0 }),
        json!({ "path": ".", "depth": -1 }),
        json!({ "path": ".", "depth": "2" }),
        json!({ "path": ".", "all": "yes" }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert!(tool.validate_input(&json!({ "path": "." }), &ctx).ok);
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());

    let result =
        tool().validate_input(&json!({ "path": other.path().display().to_string() }), &ctx);
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn default_depth_lists_only_direct_children_sorted_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("beta.txt"));
    touch(&dir.path().join("alpha.txt"));
    touch(&dir.path().join("sub").join("inner.txt"));

    let result = tool().execute(&json!({ "path": "." }), &ctx).await.unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert_eq!(text, "alpha.txt\nbeta.txt\nsub/");
    let data = result.data.unwrap();
    assert_eq!(data["depth"], json!(1));
    assert_eq!(data["depthClamped"], json!(false));
    assert_eq!(data["totalEntries"], json!(3));
}

#[tokio::test]
async fn depth_two_expands_subdirectories() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("alpha.txt"));
    touch(&dir.path().join("sub").join("inner.txt"));

    let result = tool()
        .execute(&json!({ "path": ".", "depth": 2 }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "alpha.txt\nsub/\n  inner.txt");
}

#[tokio::test]
async fn depth_above_the_cap_is_clamped_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("sub").join("inner.txt"));

    let result = tool()
        .execute(&json!({ "path": ".", "depth": 99 }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert!(text.starts_with(&format!("[depth clamped to {}]", MAX_DEPTH)));
    let data = result.data.unwrap();
    assert_eq!(data["depth"], json!(MAX_DEPTH));
    assert_eq!(data["depthClamped"], json!(true));
}

#[tokio::test]
async fn hidden_entries_are_skipped_unless_all_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join(".env"));
    touch(&dir.path().join("visible.txt"));

    let hidden = tool().execute(&json!({ "path": "." }), &ctx).await.unwrap();
    assert_eq!(hidden.assistant_text(), "visible.txt");

    let all = tool()
        .execute(&json!({ "path": ".", "all": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(all.assistant_text(), ".env\nvisible.txt");
}

#[tokio::test]
async fn a_directory_is_capped_at_one_thousand_entries_with_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let bulk = dir.path().join("bulk");
    std::fs::create_dir(&bulk).unwrap();
    for index in 0..=MAX_ENTRIES_PER_DIR {
        touch(&bulk.join(format!("f{:04}.txt", index)));
    }

    let result = tool()
        .execute(&json!({ "path": "bulk" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalEntries"], json!(MAX_ENTRIES_PER_DIR));
    assert_eq!(data["truncated"], json!(true));
    let text = result.assistant_text();
    assert!(text.contains(&format!(
        "[truncated: showing {} of {} entries]",
        MAX_ENTRIES_PER_DIR,
        MAX_ENTRIES_PER_DIR + 1
    )));
}

#[tokio::test]
async fn a_file_path_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("a.txt"));

    let result = tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("not a directory"));
}

#[tokio::test]
async fn symlinked_directories_are_listed_but_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    touch(&real.join("inner.txt"));

    if !create_dir_link(&real, &dir.path().join("link")) {
        eprintln!("skipping: this environment cannot create directory symlinks");
        return;
    }

    let result = tool()
        .execute(&json!({ "path": ".", "depth": 3 }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert!(text.contains("link/"), "the link itself must be listed");
    assert!(
        !text.contains("link/inner.txt"),
        "the link must not be followed: {}",
        text
    );
    let data = result.data.unwrap();
    let entries = data["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry["name"] == json!("link") && entry["isSymlink"] == json!(true)));
}

#[tokio::test]
async fn toolbox_injected_action_field_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("a.txt"));

    let result = tool()
        .execute(&json!({ "action": "ls", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "a.txt");
}

#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}

#[cfg(unix)]
fn create_dir_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}
