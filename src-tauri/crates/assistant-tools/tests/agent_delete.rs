use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_tools::os::agent_delete::AgentDeleteTool;
use assistant_tools::os::agent_read::file_revision;
use assistant_tools::storage::{
    InMemoryReadReceiptStore, ReadReceipt, ReadReceiptKey, ReadReceiptStore,
};
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

fn tool(receipts: Arc<dyn ReadReceiptStore>) -> AgentDeleteTool {
    AgentDeleteTool::new(receipts)
}

async fn record(receipts: &Arc<dyn ReadReceiptStore>, path: &Path) -> ReadReceiptKey {
    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        path: path.display().to_string(),
    };
    receipts
        .record(
            key.clone(),
            ReadReceipt::utf8(file_revision(path).unwrap(), "call-read"),
        )
        .await;
    key
}

async fn lookup(receipts: &Arc<dyn ReadReceiptStore>, key: &ReadReceiptKey) -> bool {
    receipts.lookup(key).await.is_some()
}

#[test]
fn schema_declares_path_and_recursive_without_force() {
    let schema = tool(store()).input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["recursive"].is_object());
    assert!(
        schema["properties"].get("force").is_none(),
        "the model must not be able to skip pre-checks"
    );
    assert!(schema["properties"].get("action").is_none());
}

#[test]
fn tool_metadata_matches_the_delete_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("a.txt");
    std::fs::write(&target, "alpha\n").unwrap();

    assert_eq!(tool(store()).id(), "delete");
    assert_eq!(tool(store()).tier(), ToolTier::Os);
    assert_eq!(tool(store()).audience(), ToolAudience::BOTH);
    let hints = tool(store()).behavior_hints();
    assert!(!hints.read_only);
    assert!(hints.destructive);
    assert!(!hints.open_world);
    assert!(
        !tool(store()).is_concurrency_safe(None),
        "delete must stay serial"
    );

    let intents = tool(store())
        .permission_intents(&json!({ "path": "a.txt" }), &ctx)
        .unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].action, "delete");
    assert_eq!(
        Path::new(&intents[0].resources[0]),
        assistant_tools::os::path::canonicalize_lenient(&target).unwrap()
    );
}

#[test]
fn the_intent_path_is_stable_across_spellings() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let relative = tool(store())
        .permission_intents(&json!({ "path": "a.txt" }), &ctx)
        .unwrap();
    let absolute = tool(store())
        .permission_intents(
            &json!({ "path": dir.path().join("a.txt").display().to_string() }),
            &ctx,
        )
        .unwrap();
    assert_eq!(relative[0].resources[0], absolute[0].resources[0]);

    let traversal = tool(store()).permission_intents(&json!({ "path": "sub/../a.txt" }), &ctx);
    assert!(
        traversal.is_err(),
        "'..' must be rejected instead of producing a second key for one path"
    );
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let cases = [
        json!({}),
        json!({ "path": 42 }),
        json!({ "path": "  " }),
        json!({ "path": "a.txt", "recursive": "yes" }),
    ];
    for input in cases {
        assert!(
            !tool(store()).validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool(store()).validate_input(&json!({ "path": "a.txt" }), &ctx),
        ValidationResult::ok()
    );
}

#[test]
fn validate_input_rejects_the_workspace_root_itself() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("keep")).unwrap();

    let spellings = [
        dir.path().display().to_string(),
        format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR),
        ".".to_string(),
    ];
    for path in spellings {
        let result = tool(store()).validate_input(&json!({ "path": path }), &ctx);
        assert!(!result.ok, "expected rejection for {}", path);
        assert!(result
            .message
            .unwrap_or_default()
            .contains("workspace root"));
    }
    assert!(
        dir.path().join("keep").exists(),
        "rejection must happen before any deletion"
    );
}

#[test]
fn validate_input_rejects_the_filesystem_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir
        .path()
        .ancestors()
        .last()
        .expect("every temp path has a filesystem root")
        .to_path_buf();
    let ctx = ToolUseContext {
        workspace_root: None,
        session_id: None,
        tool_call_id: Some("call-1".to_string()),
        ..Default::default()
    };

    let result = tool(store()).validate_input(&json!({ "path": root.display().to_string() }), &ctx);
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("filesystem root"));
}

#[test]
fn validate_input_rejects_the_home_directory_itself() {
    let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
    else {
        eprintln!("no HOME / USERPROFILE on this host; skipping");
        return;
    };
    let ctx = ToolUseContext {
        workspace_root: None,
        session_id: None,
        tool_call_id: Some("call-1".to_string()),
        ..Default::default()
    };

    let result = tool(store()).validate_input(&json!({ "path": home.display().to_string() }), &ctx);
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("home directory"));
}

#[tokio::test]
async fn deletes_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("a.txt").exists());
    let data = result.data.clone().unwrap();
    assert_eq!(data["isDirectory"], json!(false));
    assert_eq!(data["recursive"], json!(false));
    assert_eq!(data["entriesDeleted"], json!(1));
    assert_eq!(data["bytesDeleted"], json!(6));
    assert_eq!(data["truncated"], json!(false));
    assert!(result.assistant_text().starts_with("Deleted file at: "));
    assert!(result
        .assistant_text()
        .contains(data["path"].as_str().unwrap()));
}

#[tokio::test]
async fn deletes_a_directory_recursively_with_magnitude() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir_all(dir.path().join("tree/nested")).unwrap();
    std::fs::write(dir.path().join("tree/a.txt"), "12345").unwrap();
    std::fs::write(dir.path().join("tree/nested/b.txt"), "123").unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "tree", "recursive": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("tree").exists());
    let data = result.data.clone().unwrap();
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["recursive"], json!(true));
    assert_eq!(data["entriesDeleted"], json!(3));
    assert_eq!(data["bytesDeleted"], json!(8));
    assert!(result.assistant_text().contains("(3 entries, 8 bytes)"));
    assert!(result
        .assistant_text()
        .starts_with("Deleted directory at: "));
}

#[tokio::test]
async fn a_non_empty_directory_needs_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("tree")).unwrap();
    std::fs::write(dir.path().join("tree/a.txt"), "alpha\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "tree" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("recursive = true"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("tree/a.txt")).unwrap(),
        "alpha\n"
    );
}

#[tokio::test]
async fn an_empty_directory_is_deleted_without_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("empty")).unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "empty" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("empty").exists());
    let data = result.data.unwrap();
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["entriesDeleted"], json!(0));
}

#[tokio::test]
async fn a_missing_target_is_rejected_instead_of_succeeding_silently() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool(store())
        .execute(&json!({ "path": "nope.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("does not exist"));
}

#[tokio::test]
async fn deleting_a_file_link_keeps_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    if !try_symlink_file(&target, &link) {
        eprintln!("file symlinks unavailable on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "link.txt", "recursive": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("link.txt").exists());
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "alpha\n",
        "the link target must survive"
    );
    let data = result.data.unwrap();
    assert_eq!(data["isDirectory"], json!(false));
    assert_eq!(data["entriesDeleted"], json!(1));
    assert_eq!(data["bytesDeleted"], json!(0));
}

#[tokio::test]
async fn deleting_a_directory_link_keeps_its_contents() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), "keep\n").unwrap();

    for recursive in [false, true] {
        let name = format!("link-{}", recursive);
        let link = dir.path().join(&name);
        if !try_link_dir(&target, &link) {
            eprintln!("directory links unavailable on this host; skipping");
            return;
        }
        let result = tool(store())
            .execute(&json!({ "path": name, "recursive": recursive }), &ctx)
            .await
            .unwrap();
        assert!(result.ok, "{:?}", result.error);
        assert!(!link.exists(), "the link itself must be removed");
        assert_eq!(
            std::fs::read_to_string(target.join("keep.txt")).unwrap(),
            "keep\n",
            "recursive must not change link behaviour"
        );
        assert_eq!(result.data.unwrap()["isDirectory"], json!(false));
    }
}

#[tokio::test]
async fn deleting_a_dangling_link_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let link = dir.path().join("dangling.txt");
    if !try_symlink_file(&dir.path().join("missing.txt"), &link) {
        eprintln!("file symlinks unavailable on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "dangling.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!link.exists());
}

#[tokio::test]
async fn the_preview_reports_the_scale_before_approval() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::create_dir(dir.path().join("tree")).unwrap();
    std::fs::write(dir.path().join("tree/a.txt"), "12345").unwrap();

    let file = tool(store())
        .preview_for_approval(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(file.summary.contains("delete the file at"));
    assert!(file.summary.contains("6 B"));
    assert!(!file.truncated);
    assert!(file.estimated);

    let tree = tool(store())
        .preview_for_approval(&json!({ "path": "tree", "recursive": true }), &ctx)
        .await
        .unwrap();
    assert!(tree.summary.contains("recursively"));
    assert!(tree.summary.contains("1 entries"));
    assert!(tree.summary.contains("5 B"));

    assert!(
        tool(store())
            .preview_for_approval(&json!({ "path": "missing.txt" }), &ctx)
            .await
            .is_none(),
        "no preview for a target that cannot be resolved or inspected"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn a_read_only_file_is_refused_without_an_unreachable_hint() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("locked.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    if !set_readonly(&target, true) {
        eprintln!("cannot set the read-only attribute on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "locked.txt" }), &ctx)
        .await
        .unwrap();
    set_readonly(&target, false);

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("read-only"), "unexpected message: {}", error);
    assert!(
        error.contains("attribute"),
        "the message must state that delete leaves attributes untouched: {}",
        error
    );
    assert!(
        !error.contains("remove the"),
        "no tool can change attributes, so the message must not suggest doing it: {}",
        error
    );
    assert!(target.exists(), "a refused delete must not remove the file");
}

#[tokio::test]
async fn the_receipt_is_invalidated_after_delete() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let target = dir.path().join("a.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    let key = record(&receipts, &target).await;
    assert!(lookup(&receipts, &key).await);

    let result = tool(receipts.clone())
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(result.ok, "{:?}", result.error);

    assert!(
        !lookup(&receipts, &key).await,
        "a deleted path must lose its receipt"
    );
}

#[tokio::test]
async fn deleting_a_directory_invalidates_receipts_of_its_subtree() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    std::fs::create_dir(dir.path().join("tree")).unwrap();
    let child = dir.path().join("tree/a.txt");
    std::fs::write(&child, "alpha\n").unwrap();
    let key = record(&receipts, &child).await;

    let result = tool(receipts.clone())
        .execute(&json!({ "path": "tree", "recursive": true }), &ctx)
        .await
        .unwrap();
    assert!(result.ok, "{:?}", result.error);

    assert!(
        !lookup(&receipts, &key).await,
        "subtree receipts must be invalidated with the directory"
    );
}

#[cfg(unix)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(unix)]
fn try_link_dir(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn try_link_dir(target: &Path, link: &Path) -> bool {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return true;
    }
    std::process::Command::new("cmd")
        .args([
            "/c",
            "mklink",
            "/J",
            &link.display().to_string(),
            &target.display().to_string(),
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn set_readonly(path: &Path, readonly: bool) -> bool {
    let flag = if readonly { "+R" } else { "-R" };
    std::process::Command::new("attrib")
        .args([flag, &path.display().to_string()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
