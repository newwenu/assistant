use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_tools::os::agent_write::{AgentWriteTool, MAX_TRANSCODE_BYTES, MAX_WRITE_BYTES};
use assistant_tools::storage::{
    InMemoryReadReceiptStore, ReadReceipt, ReadReceiptKey, ReadReceiptStore,
};
use assistant_tools::{
    Tool, ToolAudience, ToolErrorKind, ToolResult, ToolTier, ToolUseContext, ValidationResult,
};
use serde_json::{json, Value};

fn context(root: &Path, session: Option<&str>) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: session.map(str::to_string),
        tool_call_id: Some("call-1".to_string()),
        ..Default::default()
    }
}

fn tool(receipts: Arc<dyn ReadReceiptStore>) -> AgentWriteTool {
    AgentWriteTool::new(receipts)
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

fn write_bytes_fixture(root: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(&path, content).expect("write fixture");
    path
}

/// 回执键里的路径必须与工具解析后的一致（`resolve` 会做规范化）。
fn resolved_path(tool: &AgentWriteTool, ctx: &ToolUseContext, input: &Value) -> String {
    tool.permission_intents(input, ctx).expect("resolve")[0].resources[0].clone()
}

/// 模拟一次 `read`：写入回执，供写侧继承编码（G1 / G4）。
async fn record_read_receipt(
    receipts: &Arc<dyn ReadReceiptStore>,
    ctx: &ToolUseContext,
    resolved: &str,
    encoding: &str,
    has_bom: bool,
) {
    receipts
        .record(
            ReadReceiptKey {
                session_id: ctx.session_id.clone(),
                path: resolved.to_string(),
            },
            ReadReceipt {
                revision: String::new(),
                tool_call_id: "call-read".to_string(),
                encoding: encoding.to_string(),
                has_bom,
            },
        )
        .await;
}

#[test]
fn schema_declares_path_content_and_append() {
    let schema = tool(store()).input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path", "content"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["content"].is_object());
    assert!(schema["properties"]["append"].is_object());
    assert!(schema["properties"]["encoding"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_write_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tool = tool(store());

    assert_eq!(tool.id(), "write");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(!tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(!tool.is_concurrency_safe(None), "write must stay serial");

    let intents = tool
        .permission_intents(&json!({ "path": "a.txt", "content": "x" }), &ctx)
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
        json!({ "content": "x" }),
        json!({ "path": "a.txt" }),
        json!({ "path": 42, "content": "x" }),
        json!({ "path": "  ", "content": "x" }),
        json!({ "path": "a.txt", "content": 42 }),
        json!({ "path": "a.txt", "content": "x", "append": "yes" }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool.validate_input(&json!({ "path": "a.txt", "content": "" }), &ctx),
        ValidationResult::ok(),
        "empty content is a valid write"
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let outside = other.path().join("file.txt");

    let result = tool(store()).validate_input(
        &json!({ "path": outside.display().to_string(), "content": "x" }),
        &ctx,
    );
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn creates_a_new_file_and_its_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool(store())
        .execute(
            &json!({ "path": "nested/deep/a.txt", "content": "alpha\nbeta\n" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let content = std::fs::read_to_string(dir.path().join("nested/deep/a.txt")).unwrap();
    assert_eq!(content, "alpha\nbeta\n");

    let data = result.data.clone().unwrap();
    assert_eq!(data["created"], json!(true));
    assert_eq!(data["appended"], json!(false));
    assert_eq!(data["unchanged"], json!(false));
    assert_eq!(data["lines"], json!(2));
    assert_eq!(data["bytes"], json!(11));
    assert!(result.assistant_text().starts_with("Created "));
}

#[tokio::test]
async fn overwrites_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_fixture(dir.path(), "a.txt", "old content\n");

    let result = tool(store())
        .execute(
            &json!({ "path": "a.txt", "content": "new content\n" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "new content\n"
    );
    let data = result.data.clone().unwrap();
    assert_eq!(data["created"], json!(false));
    assert_eq!(data["unchanged"], json!(false));
    assert!(result.assistant_text().starts_with("Wrote "));
}

#[tokio::test]
async fn appends_to_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_fixture(dir.path(), "a.txt", "first\n");

    let result = tool(store())
        .execute(
            &json!({ "path": "a.txt", "content": "second\n", "append": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "first\nsecond\n"
    );
    let data = result.data.clone().unwrap();
    assert_eq!(data["appended"], json!(true));
    assert_eq!(data["lines"], json!(2));
    assert!(result.assistant_text().starts_with("Appended to "));
}

#[tokio::test]
async fn append_to_a_missing_file_creates_it() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let result = tool(store())
        .execute(
            &json!({ "path": "a.txt", "content": "only\n", "append": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["created"], json!(true));
    assert_eq!(data["appended"], json!(false));
}

#[tokio::test]
async fn identical_content_is_a_no_op_and_keeps_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let path = write_fixture(dir.path(), "a.txt", "same\n");
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "a.txt", "content": "same\n" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.clone().unwrap();
    assert_eq!(data["unchanged"], json!(true));
    assert_eq!(data["created"], json!(false));
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before,
        "an idempotent write must not touch the file"
    );
    assert!(result.assistant_text().contains("nothing written"));
}

#[tokio::test]
async fn content_above_the_limit_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let content = "x".repeat(MAX_WRITE_BYTES + 1);

    let result = tool(store())
        .execute(&json!({ "path": "big.txt", "content": content }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("write limit"));
    assert!(!dir.path().join("big.txt").exists());
}

#[tokio::test]
async fn directory_target_is_rejected_without_a_misleading_hint() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("sub")).unwrap();

    let result = tool(store())
        .execute(&json!({ "path": "sub", "content": "x" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let error = result.error.unwrap_or_default();
    assert!(error.contains("is a directory"));
    assert!(
        !error.contains("mkdir"),
        "mkdir cannot fix an already existing directory, so the message must not suggest it: {}",
        error
    );
}

#[cfg(windows)]
#[tokio::test]
async fn unreadable_existing_file_is_never_overwritten() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let path = write_fixture(dir.path(), "locked.txt", "original\n");

    let guard = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("hold the fixture exclusively");

    let result = tool(store())
        .execute(
            &json!({ "path": "locked.txt", "content": "replaced\n" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("refusing to overwrite"));

    drop(guard);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_existing_file_is_never_overwritten() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let path = write_fixture(dir.path(), "locked.txt", "original\n");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

    if std::fs::read(&path).is_ok() {
        return;
    }

    let result = tool(store())
        .execute(
            &json!({ "path": "locked.txt", "content": "replaced\n" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("refusing to overwrite"));
}

#[tokio::test]
async fn preview_reports_the_existing_file_size() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_fixture(dir.path(), "a.txt", "one\ntwo\nthree\n");

    let preview = tool(store())
        .preview_for_approval(&json!({ "path": "a.txt", "content": "x" }), &ctx)
        .await
        .expect("overwrite preview");

    assert!(preview.summary.contains("覆盖 1 个现有文件"));
    assert!(preview.summary.contains("当前 3 行"));
    assert!(!preview.truncated);
    assert!(!preview.estimated);
}

#[tokio::test]
async fn preview_reports_a_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);

    let preview = tool(store())
        .preview_for_approval(&json!({ "path": "new.txt", "content": "a\nb\n" }), &ctx)
        .await
        .expect("create preview");

    assert!(preview.summary.contains("新建文件"));
    assert!(preview.summary.contains("2 行"));
}

#[tokio::test]
async fn write_records_a_read_receipt_for_the_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();

    let result = tool(receipts.clone())
        .execute(&json!({ "path": "a.txt", "content": "alpha\n" }), &ctx)
        .await
        .unwrap();

    let data = result.data.clone().unwrap();
    let path_text = data["path"].as_str().unwrap().to_string();
    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        path: path_text,
    };
    let receipt = receipts
        .lookup(&key)
        .await
        .expect("write must record a receipt for the full file");

    assert_eq!(Some(receipt.revision.as_str()), data["revision"].as_str());
    assert_eq!(receipt.tool_call_id, "call-1");
}

#[tokio::test]
async fn result_data_carries_no_error_channel_on_success() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let result: ToolResult = tool(store())
        .execute(&json!({ "path": "a.txt", "content": "x" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result.error.is_none());
    assert!(result.error_kind.is_none());
    assert_eq!(result.data.unwrap()["bytes"], Value::from(1));
}

#[tokio::test]
async fn new_file_is_written_in_the_requested_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();

    let result = tool(receipts.clone())
        .execute(
            &json!({ "path": "gbk.txt", "content": "中文内容\n", "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.clone().unwrap();
    assert_eq!(data["encoding"], json!("GBK"));
    assert_eq!(data["hasBom"], json!(false));

    let (expected, _, had_errors) = encoding_rs::GBK.encode("中文内容\n");
    assert!(!had_errors);
    assert_eq!(
        &std::fs::read(dir.path().join("gbk.txt")).unwrap()[..],
        expected.as_ref()
    );

    let key = ReadReceiptKey {
        session_id: Some("session-1".to_string()),
        path: data["path"].as_str().unwrap().to_string(),
    };
    let receipt = receipts.lookup(&key).await.expect("receipt recorded");
    assert_eq!(receipt.encoding, "GBK");
    assert!(!receipt.has_bom);
}

#[tokio::test]
async fn transcoding_an_unread_existing_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let path = write_fixture(dir.path(), "a.txt", "alpha\n");

    let result = tool(store())
        .execute(
            &json!({ "path": "a.txt", "content": "beta\n", "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result
        .error
        .unwrap_or_default()
        .contains("has not been read in this session"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "alpha\n",
        "a rejected write must leave the file untouched"
    );
}

#[tokio::test]
async fn characters_the_target_encoding_cannot_represent_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));

    let result = tool(store())
        .execute(
            &json!({ "path": "gbk.txt", "content": "emoji 😀\n", "encoding": "gbk" }),
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
    assert!(
        !dir.path().join("gbk.txt").exists(),
        "a lossy encode must not create the file"
    );
}

#[tokio::test]
async fn transcoded_writes_above_one_megabyte_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let content = "x".repeat(MAX_TRANSCODE_BYTES + 1);

    let result = tool(store())
        .execute(
            &json!({ "path": "big.txt", "content": content, "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("byte limit"));
    assert!(!dir.path().join("big.txt").exists());
}

#[tokio::test]
async fn append_inherits_the_encoding_from_the_read_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "gbk.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "GBK", false).await;

    let result = tool
        .execute(
            &json!({ "path": "gbk.txt", "content": "内容\n", "append": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.data.clone().unwrap()["encoding"], json!("GBK"));

    let written = std::fs::read(dir.path().join("gbk.txt")).unwrap();
    let (expected, _, had_errors) = encoding_rs::GBK.encode("中文内容\n");
    assert!(!had_errors);
    assert_eq!(
        &written[..],
        expected.as_ref(),
        "append must decode the existing bytes, concatenate, then re-encode as a whole"
    );
}

#[tokio::test]
async fn transcoded_overwrite_with_identical_content_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    let path = write_bytes_fixture(dir.path(), "gbk.txt", &existing);
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "gbk.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "GBK", false).await;

    let result = tool
        .execute(&json!({ "path": "gbk.txt", "content": "中文\n" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.data.clone().unwrap()["unchanged"], json!(true));
    assert_eq!(&std::fs::read(&path).unwrap()[..], existing.as_ref());
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before,
        "byte-level idempotency must not touch the file"
    );
}

#[tokio::test]
async fn utf8_bom_is_preserved_through_the_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    write_bytes_fixture(dir.path(), "a.txt", b"\xEF\xBB\xBFalpha\n");

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "a.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "UTF-8", true).await;

    let result = tool
        .execute(&json!({ "path": "a.txt", "content": "beta\n" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.clone().unwrap();
    assert_eq!(data["encoding"], json!("UTF-8"));
    assert_eq!(data["hasBom"], json!(true));
    assert_eq!(
        std::fs::read(dir.path().join("a.txt")).unwrap(),
        b"\xEF\xBB\xBFbeta\n"
    );
}

#[tokio::test]
async fn utf16_target_always_writes_a_bom() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));

    let result = tool(store())
        .execute(
            &json!({ "path": "u16.txt", "content": "hi\n", "encoding": "utf-16le" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.data.clone().unwrap()["hasBom"], json!(true));

    let written = std::fs::read(dir.path().join("u16.txt")).unwrap();
    assert!(written.starts_with(b"\xFF\xFE"));
    let text: String = written[2..]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .map(|unit| char::from_u32(unit as u32).unwrap_or('\u{FFFD}'))
        .collect();
    assert_eq!(text, "hi\n");
}

#[tokio::test]
async fn explicit_transcode_overwrites_the_existing_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "gbk.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "GBK", false).await;

    let result = tool
        .execute(
            &json!({ "path": "gbk.txt", "content": "plain\n", "encoding": "utf-8" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        std::fs::read(dir.path().join("gbk.txt")).unwrap(),
        b"plain\n",
        "an explicit encoding must win over the inherited one"
    );
}

#[tokio::test]
async fn transcoded_append_decodes_with_the_existing_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    let (existing, _, had_errors) = encoding_rs::GBK.encode("中文");
    assert!(!had_errors);
    write_bytes_fixture(dir.path(), "gbk.txt", &existing);

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "gbk.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "GBK", false).await;

    let result = tool
        .execute(
            &json!({
                "path": "gbk.txt",
                "content": "内容\n",
                "append": true,
                "encoding": "utf-8"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        String::from_utf8(std::fs::read(dir.path().join("gbk.txt")).unwrap()).unwrap(),
        "中文内容\n",
        "append must decode with the source encoding, not the target one"
    );
}

#[tokio::test]
async fn transcoded_overwrite_ignores_the_replaced_bytes_in_the_size_limit() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    let receipts = store();
    let existing = vec![b'x'; MAX_TRANSCODE_BYTES + 4096];
    write_bytes_fixture(dir.path(), "big.txt", &existing);

    let tool = tool(receipts.clone());
    let resolved = resolved_path(&tool, &ctx, &json!({ "path": "big.txt", "content": "x" }));
    record_read_receipt(&receipts, &ctx, &resolved, "GBK", false).await;

    let result = tool
        .execute(
            &json!({ "path": "big.txt", "content": "hi\n", "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let (expected, _, had_errors) = encoding_rs::GBK.encode("hi\n");
    assert!(!had_errors);
    assert_eq!(
        &std::fs::read(dir.path().join("big.txt")).unwrap()[..],
        expected.as_ref()
    );
}
