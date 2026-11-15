use assistant_tools::context::ToolUseContext;
use assistant_tools::os::fs_link::FsLinkTool;
use assistant_tools::r#trait::Tool;
use assistant_tools::result::ToolResult;
use serde_json::json;
use std::path::{Path, PathBuf};

mod support;

use support::link::{dir_link, file_link};

/// Agent 链路形态：只填 `workspace_root`（ToolBox 形态没有边界，不能用来断言越界）。
fn context_with_root(root: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        ..Default::default()
    }
}

async fn run_create(
    source: &Path,
    target: &Path,
    kind: &str,
    context: &ToolUseContext,
) -> Result<ToolResult, String> {
    FsLinkTool
        .execute(
            &json!({
                "action": "create",
                "source": source.display().to_string(),
                "target": target.display().to_string(),
                "kind": kind,
            }),
            context,
        )
        .await
}

async fn run_check(source: &Path, context: &ToolUseContext) -> Result<ToolResult, String> {
    FsLinkTool
        .execute(
            &json!({
                "action": "check",
                "source": source.display().to_string(),
            }),
            context,
        )
        .await
}

fn protected_root_for_test() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("SystemRoot").map(PathBuf::from)
    }
    #[cfg(unix)]
    {
        Some(PathBuf::from("/etc"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

#[test]
fn permission_intents_use_edit_vocabulary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let intents = FsLinkTool
        .permission_intents(
            &json!({"action": "create", "source": "a.txt", "target": "b.txt"}),
            &context,
        )
        .expect("permission intents");
    assert_eq!(intents.len(), 2);
    for intent in &intents {
        assert_eq!(intent.action, "edit");
    }
    assert_eq!(
        intents[0].resources,
        vec![dir.path().join("a.txt").display().to_string()],
        "意图必须是解析后的绝对路径，策略按绝对路径匹配"
    );
    assert_eq!(
        intents[1].resources,
        vec![dir.path().join("b.txt").display().to_string()]
    );
}

#[test]
fn permission_intents_do_not_require_link_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let intents = FsLinkTool
        .permission_intents(
            &json!({"action": "create", "source": "a.txt", "target": "b.txt"}),
            &context,
        )
        .expect("链接类型不参与授权范围");
    assert_eq!(intents.len(), 2);
}

#[test]
fn permission_intents_declare_read_for_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let intents = FsLinkTool
        .permission_intents(&json!({"action": "check", "source": "a.txt"}), &context)
        .expect("permission intents");
    assert_eq!(intents.len(), 1, "check 只读，必须声明 1 条 read");
    assert_eq!(intents[0].action, "read");
    assert_eq!(
        intents[0].resources,
        vec![dir.path().join("a.txt").display().to_string()],
        "意图必须是解析后的绝对路径"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_reports_regular_file_as_not_link() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("plain.txt");
    std::fs::write(&file, b"data").expect("write file");
    let context = context_with_root(dir.path());

    let result = run_check(&file, &context).await.expect("check");
    assert!(result.ok, "check must succeed: {:?}", result.error);
    let data = result.data.expect("data");
    assert_eq!(data["isLink"], json!(false));
    assert_eq!(data["kind"], json!(null));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_reports_missing_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let source = dir.path().join("link.txt");
    let target = dir.path().join("missing.txt");

    let result = run_create(&source, &target, "symlink", &context)
        .await
        .expect("create call");
    assert!(!result.ok, "missing target must fail");
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("does not exist"),
        "unexpected error: {:?}",
        result.error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_reports_missing_source_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let source = dir.path().join("missing_dir").join("link.txt");

    let result = run_create(&source, &target, "symlink", &context)
        .await
        .expect("create call");
    assert!(!result.ok, "missing source parent must fail");
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("parent directory"),
        "unexpected error: {:?}",
        result.error
    );
    assert!(!dir.path().join("missing_dir").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_rejects_path_outside_workspace() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let other_dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(root_dir.path());
    let target = root_dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let source = other_dir.path().join("link.txt");

    let error = run_create(&source, &target, "symlink", &context)
        .await
        .expect_err("outside path must be rejected");
    assert!(
        error.contains("outside workspace root"),
        "unexpected error: {error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_rejects_parent_dir_traversal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let source = dir.path().join("sub").join("..").join("link.txt");

    let error = run_create(&source, &target, "symlink", &context)
        .await
        .expect_err("'..' must be rejected");
    assert!(error.contains(".."), "unexpected error: {error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn create_rejects_escape_through_symlinked_directory() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let outside_dir = tempfile::tempdir().expect("tempdir");
    let escape = root_dir.path().join("escape");
    if !dir_link(outside_dir.path(), &escape) {
        return;
    }
    let context = context_with_root(root_dir.path());
    let target = root_dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let source = escape.join("link.txt");

    let error = run_create(&source, &target, "symlink", &context)
        .await
        .expect_err("escape through symlinked dir must be rejected");
    assert!(
        error.contains("outside workspace root"),
        "unexpected error: {error}"
    );
    assert!(
        !outside_dir.path().join("link.txt").exists(),
        "nothing may be written outside the workspace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_never_writes_through_existing_source_symlink() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let outside_dir = tempfile::tempdir().expect("tempdir");
    let outside_file = outside_dir.path().join("outside.txt");
    std::fs::write(&outside_file, b"data").expect("write outside file");
    let source = root_dir.path().join("escape.txt");
    if !file_link(&outside_file, &source) {
        return;
    }
    let context = context_with_root(root_dir.path());
    let target = root_dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");

    let result = run_create(&source, &target, "symlink", &context).await;
    assert!(
        result.is_err() || !result.expect("result").ok,
        "existing source symlink must not be replaced silently"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_does_not_follow_source_symlink() {
    let root_dir = tempfile::tempdir().expect("tempdir");
    let outside_dir = tempfile::tempdir().expect("tempdir");
    let outside_file = outside_dir.path().join("outside.txt");
    std::fs::write(&outside_file, b"data").expect("write outside file");
    let source = root_dir.path().join("link.txt");
    if !file_link(&outside_file, &source) {
        return;
    }
    let context = context_with_root(root_dir.path());

    let result = run_check(&source, &context).await.expect("check");
    assert!(result.ok, "check must succeed: {:?}", result.error);
    let data = result.data.expect("data");
    assert_eq!(data["isLink"], json!(true));
    assert_eq!(data["kind"], json!("symlink"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn create_symlink_and_check_reports_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let source = dir.path().join("link.txt");

    let result = run_create(&source, &target, "symlink", &context)
        .await
        .expect("create call");
    assert!(result.ok, "symlink create must succeed: {:?}", result.error);
    assert!(source
        .symlink_metadata()
        .expect("metadata")
        .file_type()
        .is_symlink());

    let checked = run_check(&source, &context).await.expect("check");
    assert_eq!(checked.data.expect("data")["isLink"], json!(true));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_rejects_protected_workspace_root() {
    let Some(protected) = protected_root_for_test() else {
        return;
    };
    let context = context_with_root(&protected);
    let source = protected.join("link.txt");
    let target = protected.join("target.txt");

    let error = run_create(&source, &target, "symlink", &context)
        .await
        .expect_err("protected workspace root must be rejected");
    assert!(
        error.contains("protected system directory"),
        "unexpected error: {error}"
    );
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn create_junction_and_check_reports_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("target_dir");
    std::fs::create_dir(&target).expect("create target dir");
    let source = dir.path().join("junction");

    let result = run_create(&source, &target, "junction", &context)
        .await
        .expect("create call");
    assert!(
        result.ok,
        "junction create must succeed: {:?}",
        result.error
    );

    let checked = run_check(&source, &context).await.expect("check");
    let data = checked.data.expect("data");
    assert_eq!(data["isLink"], json!(true));
    let reported = data["target"].as_str().unwrap_or_default();
    assert!(
        reported.ends_with("target_dir"),
        "reported target must point at the junction destination: {reported}"
    );
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn create_junction_reports_missing_target_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("missing_dir").join("target_dir");
    let source = dir.path().join("junction");

    let result = run_create(&source, &target, "junction", &context)
        .await
        .expect("create call");
    assert!(!result.ok, "missing target parent must fail");
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("does not exist"),
        "unexpected error: {:?}",
        result.error
    );
    assert!(!source.exists(), "junction must not be created");
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn create_rejects_source_on_nonexistent_drive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let context = context_with_root(dir.path());
    let target = dir.path().join("target.txt");
    std::fs::write(&target, b"data").expect("write target");
    let drive = ('D'..='Z')
        .map(|letter| format!("{}:\\", letter))
        .find(|candidate| !Path::new(candidate).exists());
    let Some(drive) = drive else {
        return;
    };
    let source = PathBuf::from(drive).join("link.txt");

    let error = run_create(&source, &target, "symlink", &context)
        .await
        .expect_err("nonexistent drive must be rejected");
    assert!(
        error.contains("no existing ancestor"),
        "unexpected error: {error}"
    );
}
