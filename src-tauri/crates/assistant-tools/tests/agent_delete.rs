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

mod support;

use support::link::{dir_link, file_link};

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
        // 回执键必须与工具解析后的路径一致（macOS 上 `/var` → `/private/var`）。
        path: assistant_tools::os::path::canonicalize_lenient(path)
            .expect("canonical path")
            .display()
            .to_string(),
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
fn schema_declares_path_recursive_and_permanent_without_force() {
    let schema = tool(store()).input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["recursive"].is_object());
    assert!(schema["properties"]["permanent"].is_object());
    assert_eq!(schema["properties"]["permanent"]["type"], json!("boolean"));
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
    assert_eq!(intents.len(), 1, "trashing needs no extra approval");
    assert_eq!(intents[0].action, "delete");
    assert_eq!(
        Path::new(&intents[0].resources[0]),
        assistant_tools::os::path::canonicalize_lenient(&target).unwrap()
    );
}

#[test]
fn permanent_mode_adds_a_purge_intent() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("a.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    let canonical = assistant_tools::os::path::canonicalize_lenient(&target).unwrap();

    let intents = tool(store())
        .permission_intents(&json!({ "path": "a.txt", "permanent": true }), &ctx)
        .unwrap();

    assert_eq!(intents.len(), 2);
    assert_eq!(intents[0].action, "delete");
    assert_eq!(intents[1].action, "purge");
    for intent in &intents {
        assert_eq!(Path::new(&intent.resources[0]), canonical);
    }
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
        json!({ "path": "a.txt", "permanent": "yes" }),
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
fn the_output_namespace_stays_read_only_for_delete() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), "log").unwrap();
    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());
    let target = output.path().join("run.log");

    let result =
        tool(store()).validate_input(&json!({ "path": target.display().to_string() }), &ctx);
    assert!(!result.ok, "输出域只读：delete 不得删除本会话输出文件");
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
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
async fn a_file_goes_to_the_recycle_bin_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("a.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    if !recycle_bin_available(dir.path()) {
        eprintln!("no usable recycle bin on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!target.exists(), "the target must leave the workspace");
    let trashed = trash_items_under(dir.path()).expect("listing works once the probe succeeded");
    assert_eq!(
        trashed.len(),
        1,
        "the file must be restorable from the recycle bin, not silently purged"
    );
    assert!(assistant_tools::os::path::same_path(
        &trashed[0].original_path(),
        &target
    ));
    purge_trash_under(dir.path());

    let data = result.data.clone().unwrap();
    assert_eq!(data["permanent"], json!(false));
    assert_eq!(data["isDirectory"], json!(false));
    assert_eq!(data["recursive"], json!(false));
    assert_eq!(data["entriesDeleted"], json!(1));
    assert_eq!(data["bytesDeleted"], json!(6));
    assert_eq!(data["truncated"], json!(false));
    assert!(
        result
            .assistant_text()
            .starts_with("Moved file to the system recycle bin: "),
        "unexpected text: {}",
        result.assistant_text()
    );
    assert!(result
        .assistant_text()
        .contains(data["path"].as_str().unwrap()));
}

#[tokio::test]
async fn a_file_is_permanently_deleted_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("a.txt");
    std::fs::write(&target, "alpha\n").unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "a.txt", "permanent": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!target.exists());
    let data = result.data.clone().unwrap();
    assert_eq!(data["permanent"], json!(true));
    assert_eq!(data["entriesDeleted"], json!(1));
    assert_eq!(data["bytesDeleted"], json!(6));
    assert!(result
        .assistant_text()
        .starts_with("Permanently deleted file at: "));
    assert!(result
        .assistant_text()
        .contains(data["path"].as_str().unwrap()));
    assert_no_trash_entry_for(&target);
}

#[tokio::test]
async fn a_directory_goes_to_the_recycle_bin_recursively() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tree = dir.path().join("tree");
    std::fs::create_dir_all(tree.join("nested")).unwrap();
    std::fs::write(tree.join("a.txt"), "12345").unwrap();
    std::fs::write(tree.join("nested/b.txt"), "123").unwrap();
    if !recycle_bin_available(dir.path()) {
        eprintln!("no usable recycle bin on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "tree", "recursive": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!tree.exists());
    let trashed = trash_items_under(dir.path()).expect("listing works once the probe succeeded");
    assert_eq!(trashed.len(), 1, "the whole tree must land in the bin");
    purge_trash_under(dir.path());

    let data = result.data.clone().unwrap();
    assert_eq!(data["permanent"], json!(false));
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["recursive"], json!(true));
    assert_eq!(data["entriesDeleted"], json!(3));
    assert_eq!(data["bytesDeleted"], json!(8));
    assert!(result.assistant_text().contains("(3 entries, 8 bytes)"));
    assert!(result
        .assistant_text()
        .starts_with("Moved directory to the system recycle bin: "));
}

#[tokio::test]
async fn a_directory_is_permanently_deleted_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tree = dir.path().join("tree");
    std::fs::create_dir_all(tree.join("nested")).unwrap();
    std::fs::write(tree.join("a.txt"), "12345").unwrap();
    std::fs::write(tree.join("nested/b.txt"), "123").unwrap();

    let result = tool(store())
        .execute(
            &json!({ "path": "tree", "recursive": true, "permanent": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!tree.exists());
    let data = result.data.clone().unwrap();
    assert_eq!(data["permanent"], json!(true));
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["entriesDeleted"], json!(3));
    assert_eq!(data["bytesDeleted"], json!(8));
    assert!(result.assistant_text().contains("(3 entries, 8 bytes)"));
    assert!(result
        .assistant_text()
        .starts_with("Permanently deleted directory at: "));
    assert_no_trash_entry_for(&tree);
}

#[tokio::test]
async fn a_non_empty_directory_needs_recursive_in_both_modes() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    for permanent in [false, true] {
        let name = format!("tree-{}", permanent);
        let tree = dir.path().join(&name);
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("a.txt"), "alpha\n").unwrap();

        let result = tool(store())
            .execute(&json!({ "path": name, "permanent": permanent }), &ctx)
            .await
            .unwrap();

        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
        assert!(result
            .error
            .unwrap_or_default()
            .contains("recursive = true"));
        assert_eq!(
            std::fs::read_to_string(tree.join("a.txt")).unwrap(),
            "alpha\n",
            "the pre-check must run before any move or delete"
        );
    }
}

#[tokio::test]
async fn an_empty_directory_goes_to_the_recycle_bin_without_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    if !recycle_bin_available(dir.path()) {
        eprintln!("no usable recycle bin on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "empty" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("empty").exists());
    purge_trash_under(dir.path());
    let data = result.data.unwrap();
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["permanent"], json!(false));
    assert_eq!(data["entriesDeleted"], json!(0));
}

#[tokio::test]
async fn an_empty_directory_is_permanently_deleted_without_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("empty")).unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "empty", "permanent": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!dir.path().join("empty").exists());
    let data = result.data.unwrap();
    assert_eq!(data["isDirectory"], json!(true));
    assert_eq!(data["permanent"], json!(true));
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
    if !file_link(&target, &link) {
        eprintln!("file symlinks unavailable on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(
            &json!({ "path": "link.txt", "recursive": true, "permanent": true }),
            &ctx,
        )
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
        if !dir_link(&target, &link) {
            eprintln!("directory links unavailable on this host; skipping");
            return;
        }
        let result = tool(store())
            .execute(
                &json!({ "path": name, "recursive": recursive, "permanent": true }),
                &ctx,
            )
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
    if !file_link(&dir.path().join("missing.txt"), &link) {
        eprintln!("file symlinks unavailable on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "dangling.txt", "permanent": true }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert!(!link.exists());
}

#[tokio::test]
async fn the_preview_reports_the_scale_and_the_destination() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::create_dir(dir.path().join("tree")).unwrap();
    std::fs::write(dir.path().join("tree/a.txt"), "12345").unwrap();

    let file = tool(store())
        .preview_for_approval(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(file.summary.contains("move the file at"));
    assert!(file.summary.contains("system recycle bin"));
    assert!(file.summary.contains("6 B"));
    assert!(!file.truncated);
    assert!(file.estimated);

    let purged_file = tool(store())
        .preview_for_approval(&json!({ "path": "a.txt", "permanent": true }), &ctx)
        .await
        .unwrap();
    assert!(purged_file
        .summary
        .contains("permanently delete the file at"));
    assert!(!purged_file.summary.contains("recycle bin"));

    let tree = tool(store())
        .preview_for_approval(&json!({ "path": "tree", "recursive": true }), &ctx)
        .await
        .unwrap();
    assert!(tree.summary.contains("move the directory at"));
    assert!(tree.summary.contains("1 entries"));
    assert!(tree.summary.contains("5 B"));
    assert!(tree.summary.contains("system recycle bin"));

    let purged_tree = tool(store())
        .preview_for_approval(
            &json!({ "path": "tree", "recursive": true, "permanent": true }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(purged_tree
        .summary
        .contains("permanently delete the directory at"));
    assert!(!purged_tree.summary.contains("recycle bin"));

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
async fn a_read_only_file_is_refused_in_permanent_mode() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("locked.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    if !set_readonly(&target, true) {
        eprintln!("cannot set the read-only attribute on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "locked.txt", "permanent": true }), &ctx)
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

#[cfg(windows)]
#[tokio::test]
async fn a_read_only_file_is_moved_to_the_recycle_bin() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let target = dir.path().join("locked.txt");
    std::fs::write(&target, "alpha\n").unwrap();
    if !set_readonly(&target, true) {
        eprintln!("cannot set the read-only attribute on this host; skipping");
        return;
    }
    if !recycle_bin_available(dir.path()) {
        eprintln!("no usable recycle bin on this host; skipping");
        return;
    }

    let result = tool(store())
        .execute(&json!({ "path": "locked.txt" }), &ctx)
        .await
        .unwrap();

    assert!(
        result.ok,
        "moving does not touch attributes, so the read-only bit must not block it: {:?}",
        result.error
    );
    assert!(!target.exists());
    purge_trash_under(dir.path());
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
        .execute(&json!({ "path": "a.txt", "permanent": true }), &ctx)
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
        .execute(
            &json!({ "path": "tree", "recursive": true, "permanent": true }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.ok, "{:?}", result.error);

    assert!(
        !lookup(&receipts, &key).await,
        "subtree receipts must be invalidated with the directory"
    );
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

/// 回收站里的条目（只统计落在测试目录内的，避免被本机其它文件干扰）。
///
/// 返回 `None` 表示本机没有列举能力（`trash` 的 `os_limited` 只覆盖 Windows /
/// freedesktop，macOS 缺失），由调用方显式跳过——空 `Vec` 会被误读成「回收站里没有条目」。
#[cfg(any(windows, target_os = "linux"))]
fn trash_items_under(dir: &Path) -> Option<Vec<trash::TrashItem>> {
    let items = trash::os_limited::list().ok()?;
    let root = assistant_tools::os::path::canonicalize_lenient(dir).ok()?;
    Some(
        items
            .into_iter()
            .filter(|item| {
                assistant_tools::os::path::is_component_prefix(&item.original_path(), &root)
            })
            .collect(),
    )
}

#[cfg(not(any(windows, target_os = "linux")))]
fn trash_items_under(_dir: &Path) -> Option<Vec<trash::TrashItem>> {
    None
}

/// 清掉测试留在回收站里的条目，避免污染开发机与后续用例。
#[cfg(any(windows, target_os = "linux"))]
fn purge_trash_under(dir: &Path) {
    let Some(items) = trash_items_under(dir) else {
        return;
    };
    assert!(
        trash::os_limited::purge_all(&items).is_ok(),
        "cleanup must leave no test leftovers in the recycle bin"
    );
}

#[cfg(not(any(windows, target_os = "linux")))]
fn purge_trash_under(_dir: &Path) {}

/// 探测该卷是否真的能用回收站：写探针 → 移入回收站 → 清掉探针。
///
/// 返回 false 时用例直接跳过：卷没有 `$Recycle.Bin`（CI 精简镜像、网络盘）时，
/// 「文件进了回收站」是环境前提而不是回归信号。
#[cfg(any(windows, target_os = "linux"))]
fn recycle_bin_available(dir: &Path) -> bool {
    let probe = dir.join(format!("trash-probe-{}.tmp", std::process::id()));
    if std::fs::write(&probe, b"probe").is_err() {
        return false;
    }
    let moved = std::panic::catch_unwind(|| trash::delete(&probe));
    if !matches!(moved, Ok(Ok(()))) {
        let _ = std::fs::remove_file(&probe);
        return false;
    }
    let Some(items) = trash_items_under(dir) else {
        return false;
    };
    let probes: Vec<_> = items
        .into_iter()
        .filter(|item| assistant_tools::os::path::same_path(&item.original_path(), &probe))
        .collect();
    trash::os_limited::purge_all(&probes).is_ok()
}

/// macOS 能移入回收站，但 `trash` 不提供列举 / 清理，探针会永久留在用户的废纸篓里。
/// 因此不做探测，直接跳过——宁可少覆盖，也不污染开发机。
#[cfg(not(any(windows, target_os = "linux")))]
fn recycle_bin_available(_dir: &Path) -> bool {
    false
}

/// 永久模式不得留下回收站条目（证明没被静默改成移动）。
///
/// 无法列举回收站时跳过并打印原因：跳过必须是可见的，否则这条断言会在不支持列举的
/// 平台上凭空消失，回归时无人察觉。
fn assert_no_trash_entry_for(target: &Path) {
    let Some(parent) = target.parent() else {
        return;
    };
    let Some(items) = trash_items_under(parent) else {
        eprintln!("recycle-bin listing unavailable on this host; skipping the trash assertion");
        return;
    };
    assert!(
        !items
            .iter()
            .any(|item| assistant_tools::os::path::same_path(&item.original_path(), target)),
        "permanent mode must not leave '{}' in the recycle bin",
        target.display()
    );
}
