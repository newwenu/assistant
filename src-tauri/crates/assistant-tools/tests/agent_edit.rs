use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_tools::os::agent_edit::AgentEditTool;
use assistant_tools::os::agent_write::AgentWriteTool;
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

fn tool(receipts: Arc<dyn ReadReceiptStore>) -> AgentEditTool {
    AgentEditTool::new(receipts)
}

fn store() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

fn write_fixture(root: &Path, name: &str, content: &str) -> PathBuf {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read file")
}

/// 模拟「本会话已读过该文件」的回执。
async fn grant_receipt(
    receipts: &Arc<dyn ReadReceiptStore>,
    root: &Path,
    name: &str,
    session: Option<&str>,
) {
    // 回执键与工具解析结果一致：`resolve` 会归一（macOS 上 `/var` → `/private/var`）。
    let resolved = assistant_tools::os::path::canonicalize_lenient(&root.join(name))
        .expect("canonical fixture");
    let revision = assistant_tools::os::agent_read::file_revision(&resolved).unwrap();
    receipts
        .record(
            ReadReceiptKey {
                session_id: session.map(str::to_string),
                path: resolved.display().to_string(),
            },
            assistant_tools::storage::ReadReceipt::utf8(revision, "call-read"),
        )
        .await;
}

/// 与 [`grant_receipt`] 相同，但回执携带 `read` 实际发现的编码 / BOM（上级 §14.5）。
async fn grant_encoded_receipt(
    receipts: &Arc<dyn ReadReceiptStore>,
    root: &Path,
    name: &str,
    session: Option<&str>,
    encoding: &str,
    has_bom: bool,
) {
    // 回执键与工具解析结果一致：`resolve` 会归一（macOS 上 `/var` → `/private/var`）。
    let resolved = assistant_tools::os::path::canonicalize_lenient(&root.join(name))
        .expect("canonical fixture");
    let revision = assistant_tools::os::agent_read::file_revision(&resolved).unwrap();
    receipts
        .record(
            ReadReceiptKey {
                session_id: session.map(str::to_string),
                path: resolved.display().to_string(),
            },
            assistant_tools::storage::ReadReceipt {
                revision,
                tool_call_id: "call-read".to_string(),
                encoding: encoding.to_string(),
                has_bom,
            },
        )
        .await;
}

fn write_bytes_fixture(root: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

#[test]
fn schema_declares_the_edit_contract() {
    let schema = tool(store()).input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(
        schema["required"],
        json!(["path", "old_string", "new_string"])
    );
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["old_string"].is_object());
    assert!(schema["properties"]["new_string"].is_object());
    assert!(schema["properties"]["replace_all"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_edit_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tool = tool(store());

    assert_eq!(tool.id(), "edit");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(!tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(!tool.is_concurrency_safe(None), "edit must stay serial");

    let intents = tool
        .permission_intents(
            &json!({ "path": "a.txt", "old_string": "x", "new_string": "y" }),
            &ctx,
        )
        .unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].action, "edit");
    assert!(Path::new(&intents[0].resources[0]).is_absolute());
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tool = tool(store());

    let cases = [
        json!({}),
        json!({ "path": "a.txt", "new_string": "y" }),
        json!({ "path": "a.txt", "old_string": "x" }),
        json!({ "old_string": "x", "new_string": "y" }),
        json!({ "path": 42, "old_string": "x", "new_string": "y" }),
        json!({ "path": "  ", "old_string": "x", "new_string": "y" }),
        json!({ "path": "a.txt", "old_string": 42, "new_string": "y" }),
        json!({ "path": "a.txt", "old_string": "x", "new_string": 42 }),
        json!({ "path": "a.txt", "old_string": "x", "new_string": "y", "replace_all": "yes" }),
        json!({ "path": "a.txt", "old_string": "", "new_string": "y" }),
        json!({ "path": "a.txt", "old_string": "x", "new_string": "x" }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool.validate_input(
            &json!({ "path": "a.txt", "old_string": "x", "new_string": "" }),
            &ctx
        ),
        ValidationResult::ok(),
        "an empty new_string is a valid deletion"
    );
}

#[test]
fn the_output_namespace_stays_read_only_for_edit() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), "old\n").unwrap();
    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());
    let target = output.path().join("run.log");

    let result = tool(store()).validate_input(
        &json!({
            "path": target.display().to_string(),
            "old_string": "old",
            "new_string": "new"
        }),
        &ctx,
    );
    assert!(!result.ok, "输出域只读：edit 不得改写本会话输出文件");
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let outside = other.path().join("file.txt");

    let result = tool(store()).validate_input(
        &json!({
            "path": outside.display().to_string(),
            "old_string": "x",
            "new_string": "y"
        }),
        &ctx,
    );
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn rejects_an_unread_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_fixture(dir.path(), "a.txt", "alpha\n");

    let result = tool(store())
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("has not been read"));
    assert_eq!(read(&dir.path().join("a.txt")), "alpha\n");
}

#[tokio::test]
async fn rejects_a_stale_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let path = write_fixture(dir.path(), "a.txt", "alpha\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    std::fs::write(&path, "alpha\nexternal change\n").unwrap();

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert!(result.error.unwrap_or_default().contains("changed after"));
    assert_eq!(read(&path), "alpha\nexternal change\n");
}

#[tokio::test]
async fn write_then_edit_is_not_blocked_by_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));

    let written = AgentWriteTool::new(receipts.clone())
        .execute(&json!({ "path": "a.txt", "content": "alpha\n" }), &ctx)
        .await
        .unwrap();
    assert!(written.ok);

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "write must count as having seen the file");
    assert_eq!(read(&dir.path().join("a.txt")), "ALPHA\n");
}

#[tokio::test]
async fn replaces_a_unique_match_and_reports_line_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\nbeta\ngamma\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "beta\ngamma", "new_string": "BETA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(read(&dir.path().join("a.txt")), "alpha\nBETA\n");

    let data = result.data.clone().unwrap();
    assert_eq!(data["startLine"], json!(2));
    assert_eq!(data["oldEndLine"], json!(3));
    assert_eq!(data["newEndLine"], json!(2));
    assert_eq!(data["matchCount"], json!(1));
    assert_eq!(data["tier"], json!("exact"));
    assert!(data["revision"].is_string());

    let diff = data["diff"].as_str().unwrap();
    assert!(diff.contains("- beta"));
    assert!(diff.contains("- gamma"));
    assert!(diff.contains("+ BETA"));
}

#[tokio::test]
async fn ambiguous_match_is_rejected_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "a\nb\na\nb\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "a\nb", "new_string": "X" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("matches 2 locations"));
    assert!(error.contains("replace_all"));
    assert!(
        error.contains("1: a"),
        "ambiguity must show contexts: {}",
        error
    );
    assert_eq!(read(&dir.path().join("a.txt")), "a\nb\na\nb\n");
}

#[tokio::test]
async fn replace_all_rewrites_every_match() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "a\nb\na\nb\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({
                "path": "a.txt",
                "old_string": "a\nb",
                "new_string": "X",
                "replace_all": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(read(&dir.path().join("a.txt")), "X\nX\n");
    let data = result.data.unwrap();
    assert_eq!(data["matchCount"], json!(2));
    assert_eq!(data["startLine"], json!(1));
}

#[tokio::test]
async fn strips_line_number_prefixes_copied_from_read() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\nbeta\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({
                "path": "a.txt",
                "old_string": "     1\talpha\n     2\tbeta",
                "new_string": "ALPHA\nBETA"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(read(&dir.path().join("a.txt")), "ALPHA\nBETA\n");
    assert_eq!(result.data.unwrap()["tier"], json!("prefix-stripped"));
}

#[tokio::test]
async fn tolerates_trailing_whitespace_drift() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha   \nbeta\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha\nbeta", "new_string": "ALPHA\nBETA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(read(&dir.path().join("a.txt")), "ALPHA\nBETA\n");
    assert_eq!(result.data.unwrap()["tier"], json!("trim-end"));
}

#[tokio::test]
async fn tolerates_indentation_drift() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "\talpha\n\tbeta\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({
                "path": "a.txt",
                "old_string": "  alpha\n  beta",
                "new_string": "\tALPHA\n\tBETA"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(read(&dir.path().join("a.txt")), "\tALPHA\n\tBETA\n");
    assert_eq!(result.data.unwrap()["tier"], json!("trim"));
}

#[tokio::test]
async fn normalized_match_rewrites_the_original_fragment() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "it\u{2019}s fine\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "it's fine", "new_string": "it is fine" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(read(&dir.path().join("a.txt")), "it is fine\n");
    assert_eq!(result.data.unwrap()["tier"], json!("normalized"));
}

#[tokio::test]
async fn missing_match_reports_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\nbeta\ngamma\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "zzz", "new_string": "y" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("was not found"));
    assert!(error.contains("call read again"));
    assert_eq!(read(&dir.path().join("a.txt")), "alpha\nbeta\ngamma\n");
}

#[tokio::test]
async fn no_change_is_rejected_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let path = write_fixture(dir.path(), "a.txt", "alpha\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "  alpha", "new_string": "alpha" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    let error = result.error.unwrap_or_default();
    assert!(error.contains("nothing to change"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
}

#[tokio::test]
async fn crlf_files_stay_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\r\nbeta\r\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "beta", "new_string": "BETA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let content = read(&dir.path().join("a.txt"));
    assert_eq!(content, "alpha\r\nBETA\r\n");
    assert_eq!(content.matches("\r\n").count(), 2);
    assert!(!content.contains("\n\n"));
}

#[tokio::test]
async fn write_is_atomic_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.ok, "{:?}", result.error);

    let entries: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries, vec!["a.txt".to_string()]);
}

#[tokio::test]
async fn rejects_missing_files_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    std::fs::create_dir(dir.path().join("sub")).unwrap();

    let missing = tool(receipts.clone())
        .execute(
            &json!({ "path": "nope.txt", "old_string": "x", "new_string": "y" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!missing.ok);
    assert!(missing.error.unwrap_or_default().contains("does not exist"));

    let directory = tool(receipts)
        .execute(
            &json!({ "path": "sub", "old_string": "x", "new_string": "y" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!directory.ok);
    assert!(directory.error.unwrap_or_default().contains("directory"));
}

#[tokio::test]
async fn preview_shows_a_diff_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let preview = tool(store())
        .preview_for_approval(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .expect("edit must preview");

    assert!(preview.summary.contains("a.txt"));
    assert!(preview.summary.contains("- alpha"));
    assert!(preview.summary.contains("+ ALPHA"));
    assert!(preview.estimated);
}

#[tokio::test]
async fn edit_refreshes_the_receipt_for_the_next_turn() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_fixture(dir.path(), "a.txt", "alpha\n");
    grant_receipt(&receipts, dir.path(), "a.txt", Some("session-1")).await;

    let first = tool(receipts.clone())
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(first.ok);

    let second = tool(receipts)
        .execute(
            &json!({ "path": "a.txt", "old_string": "ALPHA", "new_string": "ALPHA2" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(second.ok, "{:?}", second.error);
    assert_eq!(read(&dir.path().join("a.txt")), "ALPHA2\n");
}

#[tokio::test]
async fn gbk_files_stay_gbk_after_editing() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n第二行\n");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);
    grant_encoded_receipt(
        &receipts,
        dir.path(),
        "gbk.txt",
        Some("session-1"),
        "GBK",
        false,
    )
    .await;

    let result = tool(receipts.clone())
        .execute(
            &json!({ "path": "gbk.txt", "old_string": "第二行", "new_string": "改过的行" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let data = result.data.clone().unwrap();
    assert_eq!(data["encoding"], json!("GBK"));
    assert_eq!(data["hasBom"], json!(false));

    let (expected, _, had_errors) = encoding_rs::GBK.encode("中文\n改过的行\n");
    assert!(!had_errors);
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        expected.as_ref()
    );

    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        path: data["path"].as_str().unwrap().to_string(),
    };
    let receipt = receipts.lookup(&key).await.expect("receipt refreshed");
    assert_eq!(receipt.encoding, "GBK");
    assert!(!receipt.has_bom);
}

#[tokio::test]
async fn utf8_bom_files_keep_their_bom_after_editing() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    write_bytes_fixture(dir.path(), "a.txt", b"\xEF\xBB\xBFalpha\n");
    grant_encoded_receipt(
        &receipts,
        dir.path(),
        "a.txt",
        Some("session-1"),
        "UTF-8",
        true,
    )
    .await;

    let result = tool(receipts.clone())
        .execute(
            &json!({ "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.data.clone().unwrap()["hasBom"], json!(true));
    assert_eq!(
        std::fs::read(dir.path().join("a.txt")).unwrap(),
        b"\xEF\xBB\xBFALPHA\n"
    );

    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        path: result.data.clone().unwrap()["path"]
            .as_str()
            .unwrap()
            .to_string(),
    };
    let receipt = receipts.lookup(&key).await.expect("receipt refreshed");
    assert!(receipt.has_bom, "the refreshed receipt must report the BOM");
}

#[tokio::test]
async fn characters_the_file_encoding_cannot_represent_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    let path = write_bytes_fixture(dir.path(), "gbk.txt", &existing);
    grant_encoded_receipt(
        &receipts,
        dir.path(),
        "gbk.txt",
        Some("session-1"),
        "GBK",
        false,
    )
    .await;

    let result = tool(receipts)
        .execute(
            &json!({ "path": "gbk.txt", "old_string": "中文", "new_string": "表情 😀" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("cannot be encoded as GBK"));
    assert_eq!(
        &std::fs::read(&path).unwrap()[..],
        existing.as_ref(),
        "a lossy encode must leave the file untouched"
    );
}

#[tokio::test]
async fn footer_copied_into_old_string_is_diagnosed() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);
    grant_encoded_receipt(
        &receipts,
        dir.path(),
        "gbk.txt",
        Some("session-1"),
        "GBK",
        false,
    )
    .await;

    let with_footer = tool(receipts.clone())
        .execute(
            &json!({
                "path": "gbk.txt",
                "old_string": "中文\n[encoding: GBK]",
                "new_string": "改过"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!with_footer.ok);
    let error = with_footer.error.unwrap_or_default();
    assert!(error.contains("[encoding: GBK]"));
    assert!(error.contains("not part of the file"));
    assert!(error.contains("Remove it and retry"));
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        existing.as_ref()
    );

    let retried = tool(receipts)
        .execute(
            &json!({ "path": "gbk.txt", "old_string": "中文", "new_string": "改过" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(retried.ok, "{:?}", retried.error);

    let (expected, _, had_errors) = encoding_rs::GBK.encode("改过\n");
    assert!(!had_errors);
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        expected.as_ref()
    );
}

#[tokio::test]
async fn a_real_footer_shaped_last_line_still_matches_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let (existing, _, had_errors) = encoding_rs::GBK.encode("alpha\n[encoding: GBK]\n");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);
    grant_encoded_receipt(
        &receipts,
        dir.path(),
        "gbk.txt",
        Some("session-1"),
        "GBK",
        false,
    )
    .await;

    let result = tool(receipts)
        .execute(
            &json!({
                "path": "gbk.txt",
                "old_string": "[encoding: GBK]",
                "new_string": "[encoding: UTF-8]"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        result.ok,
        "footer detection must only affect diagnostics, never the match itself: {:?}",
        result.error
    );
    let (expected, _, had_errors) = encoding_rs::GBK.encode("alpha\n[encoding: UTF-8]\n");
    assert!(!had_errors);
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        expected.as_ref()
    );
}

#[tokio::test]
async fn non_utf8_files_still_require_a_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);

    let result = tool(receipts)
        .execute(
            &json!({ "path": "gbk.txt", "old_string": "中文", "new_string": "改过" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    let error = result.error.unwrap_or_default();
    assert!(error.contains("has not been read in this session"));
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        existing.as_ref()
    );
}

#[tokio::test]
async fn a_file_above_the_whole_file_limit_is_rejected_with_an_edit_only_message() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = store();
    let ctx = context(dir.path(), Some("session-1"));
    let content = "x".repeat(16 * 1024 * 1024 + 1);
    std::fs::write(dir.path().join("big.txt"), &content).unwrap();

    let result = tool(receipts)
        .execute(
            &json!({ "path": "big.txt", "old_string": "x", "new_string": "y" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("limit for edit"));
    assert!(
        !error.contains("read") && !error.contains("grep"),
        "read no longer shares this gate, so the message must not point at it: {}",
        error
    );
}
