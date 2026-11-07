use std::path::Path;
use std::sync::Arc;

use assistant_tools::os::agent_read::{
    AgentReadTool, LINE_NUMBER_WIDTH, LINE_TRUNCATED_MARKER, MAX_LINE_CHARS,
};
use assistant_tools::storage::{InMemoryReadReceiptStore, ReadReceiptStore};
use assistant_tools::{
    Tool, ToolAudience, ToolErrorKind, ToolResult, ToolTier, ToolUseContext, ValidationResult,
};
use serde_json::{json, Value};

fn context(root: &Path, session: Option<&str>) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: session.map(str::to_string),
        tool_call_id: Some("call-1".to_string()),
    }
}

fn tool() -> AgentReadTool {
    tool_with_store().0
}

fn tool_with_store() -> (AgentReadTool, Arc<InMemoryReadReceiptStore>) {
    let store = Arc::new(InMemoryReadReceiptStore::new());
    (AgentReadTool::new(store.clone()), store)
}

fn write(root: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn write_bytes(root: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn prefix(number: usize) -> String {
    format!("{:>width$}\t", number, width = LINE_NUMBER_WIDTH)
}

#[test]
fn schema_declares_path_offset_limit_and_encoding() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["path"]));
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["offset"].is_object());
    assert!(schema["properties"]["limit"].is_object());
    assert!(schema["properties"]["encoding"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_read_contract() {
    let tool = tool();
    assert_eq!(tool.id(), "read");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(tool.is_concurrency_safe(None));
    assert_eq!(
        tool.permission_intents(&json!({ "path": "a.txt" }), &ToolUseContext::default()),
        Ok(vec![])
    );
}

#[test]
fn validate_input_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tool = tool();

    let cases = [
        json!({}),
        json!({ "path": 42 }),
        json!({ "path": "  " }),
        json!({ "path": "a.txt", "offset": -1 }),
        json!({ "path": "a.txt", "offset": "1" }),
        json!({ "path": "a.txt", "limit": 0 }),
        json!({ "path": "a.txt", "limit": "10" }),
        json!({ "path": "a.txt", "encoding": 42 }),
        json!({ "path": "a.txt", "encoding": "bogus-label" }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert_eq!(
        tool.validate_input(&json!({ "path": "a.txt", "offset": 0 }), &ctx),
        ValidationResult::ok()
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let outside = other.path().join("file.txt");

    let result = tool().validate_input(&json!({ "path": outside.display().to_string() }), &ctx);
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn missing_path_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let result = tool().execute(&json!({}), &ctx).await.unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("path"));
}

#[tokio::test]
async fn reads_the_whole_file_with_line_number_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "alpha\nbeta\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert_eq!(text, format!("{}alpha\n{}beta\n", prefix(1), prefix(2)));

    let data = result.data.unwrap();
    assert_eq!(data["startLine"], json!(1));
    assert_eq!(data["endLine"], json!(2));
    assert_eq!(data["totalLines"], json!(2));
    assert_eq!(data["truncated"], json!(false));
    assert_eq!(data["deduplicated"], json!(false));
}

#[tokio::test]
async fn offset_and_limit_select_a_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "one\ntwo\nthree\nfour\nfive\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt", "offset": 2, "limit": 2 }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        result.assistant_text(),
        format!("{}two\n{}three\n", prefix(2), prefix(3))
    );
    let data = result.data.unwrap();
    assert_eq!(data["startLine"], json!(2));
    assert_eq!(data["endLine"], json!(3));
    assert_eq!(data["totalLines"], json!(5));
}

#[tokio::test]
async fn offset_zero_is_treated_as_line_one() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "one\ntwo\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt", "offset": 0 }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        result.assistant_text(),
        format!("{}one\n{}two\n", prefix(1), prefix(2))
    );
}

#[tokio::test]
async fn limit_zero_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "one\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt", "limit": 0 }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("limit"));
}

#[tokio::test]
async fn offset_beyond_eof_reports_the_file_length() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "one\ntwo\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt", "offset": 9 }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result.assistant_text().contains("file has 2 lines"));
    let data = result.data.unwrap();
    assert_eq!(data["endLine"], Value::Null);
}

#[tokio::test]
async fn binary_files_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::write(dir.path().join("bin.dat"), b"abc\0def").unwrap();

    let result = tool()
        .execute(&json!({ "path": "bin.dat" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("binary"));
}

#[tokio::test]
async fn directories_are_rejected_with_a_hint_to_ls() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    std::fs::create_dir(dir.path().join("sub")).unwrap();

    let result = tool()
        .execute(&json!({ "path": "sub" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert!(result.error.unwrap_or_default().contains("use ls"));
}

#[tokio::test]
async fn empty_file_returns_an_empty_success() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "empty.txt", "");

    let result = tool()
        .execute(&json!({ "path": "empty.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "");
    let data = result.data.unwrap();
    assert_eq!(data["totalLines"], json!(0));
    assert_eq!(data["endLine"], Value::Null);
}

#[tokio::test]
async fn long_lines_are_truncated_with_a_marker() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let long_line = "x".repeat(MAX_LINE_CHARS + 100);
    write(dir.path(), "long.txt", &long_line);

    let result = tool()
        .execute(&json!({ "path": "long.txt" }), &ctx)
        .await
        .unwrap();

    let text = result.assistant_text();
    assert!(text.ends_with(&format!("{}\n", LINE_TRUNCATED_MARKER)));
    assert!(text.contains(&"x".repeat(MAX_LINE_CHARS)));
    assert!(!text.contains(&long_line));
}

#[tokio::test]
async fn line_cap_truncates_and_reports_the_real_total() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let content: String = (1..=2500).map(|n| format!("line {}\n", n)).collect();
    write(dir.path(), "big.txt", &content);

    let result = tool()
        .execute(&json!({ "path": "big.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result
        .assistant_text()
        .contains("showing 2000 of 2500 lines"));
    let data = result.data.unwrap();
    assert_eq!(data["truncated"], json!(true));
    assert_eq!(data["totalLines"], json!(2500));
}

#[tokio::test]
async fn byte_cap_truncates_and_reports_the_real_total() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let line = "y".repeat(600);
    let content: String = (1..=2000).map(|_| format!("{}\n", line)).collect();
    write(dir.path(), "wide.txt", &content);

    let result = tool()
        .execute(&json!({ "path": "wide.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result.assistant_text().contains("bytes]"));
    let data = result.data.unwrap();
    assert_eq!(data["truncated"], json!(true));
    assert_eq!(data["totalLines"], json!(2000));
}

#[tokio::test]
async fn unchanged_reads_in_the_same_session_are_deduplicated() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    write(dir.path(), "a.txt", "alpha\nbeta\n");

    let tool = tool();
    let first = tool
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert_eq!(first.data.as_ref().unwrap()["deduplicated"], json!(false));
    assert!(first.assistant_text().contains("alpha"));

    let second = tool
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    let data = second.data.as_ref().unwrap();
    assert_eq!(data["deduplicated"], json!(true));
    assert!(!second.assistant_text().contains("alpha"));
    assert!(second.assistant_text().contains("Already returned"));
}

#[tokio::test]
async fn a_different_session_is_not_suppressed() {
    let dir = tempfile::tempdir().unwrap();
    let first_ctx = context(dir.path(), Some("session-1"));
    let second_ctx = context(dir.path(), Some("session-2"));
    write(dir.path(), "a.txt", "alpha\n");

    let tool = tool();
    tool.execute(&json!({ "path": "a.txt" }), &first_ctx)
        .await
        .unwrap();
    let second = tool
        .execute(&json!({ "path": "a.txt" }), &second_ctx)
        .await
        .unwrap();

    assert_eq!(second.data.as_ref().unwrap()["deduplicated"], json!(false));
    assert!(second.assistant_text().contains("alpha"));
}

#[tokio::test]
async fn a_changed_revision_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    write(dir.path(), "a.txt", "alpha\n");

    let tool = tool();
    tool.execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    write(dir.path(), "a.txt", "alpha\nbeta\n");

    let refreshed = tool
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        refreshed.data.as_ref().unwrap()["deduplicated"],
        json!(false)
    );
    assert!(refreshed.assistant_text().contains("beta"));
}

#[tokio::test]
async fn different_ranges_are_not_deduplicated() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    write(dir.path(), "a.txt", "one\ntwo\nthree\nfour\n");

    let tool = tool();
    tool.execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    let ranged = tool
        .execute(&json!({ "path": "a.txt", "offset": 3, "limit": 2 }), &ctx)
        .await
        .unwrap();

    assert_eq!(ranged.data.as_ref().unwrap()["deduplicated"], json!(false));
    assert!(ranged.assistant_text().contains("three"));
}

#[tokio::test]
async fn toolbox_injected_action_field_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "alpha\n");

    let result = tool()
        .execute(&json!({ "action": "read", "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result.assistant_text().contains("alpha"));
}

#[tokio::test]
async fn result_is_a_tool_result_with_ok_channel() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "alpha\n");

    let result: ToolResult = tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert!(result.error.is_none());
}

#[tokio::test]
async fn utf8_without_bom_has_no_encoding_footer() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "alpha\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.assistant_text().contains("[encoding:"));
}

#[tokio::test]
async fn utf8_with_bom_marks_the_footer_and_strips_the_bom() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_bytes(dir.path(), "a.txt", b"\xEF\xBB\xBFalpha\n");

    let result = tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert_eq!(text, format!("{}alpha\n[encoding: UTF-8 (bom)]", prefix(1)));
    assert!(!text.contains('\u{FEFF}'));
}

#[tokio::test]
async fn gbk_with_explicit_encoding_is_decoded_and_labelled() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let (bytes, _, had_errors) = encoding_rs::GBK.encode("中文内容\n");
    assert!(!had_errors);
    write_bytes(dir.path(), "gbk.txt", &bytes);

    let result = tool()
        .execute(&json!({ "path": "gbk.txt", "encoding": "gbk" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert_eq!(text, format!("{}中文内容\n[encoding: GBK]", prefix(1)));
}

#[tokio::test]
async fn gbk_without_encoding_is_guessed() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let (bytes, _, had_errors) = encoding_rs::GBK.encode("系统找不到指定的路径。");
    assert!(!had_errors);
    write_bytes(dir.path(), "gbk.txt", &bytes);

    let result = tool()
        .execute(&json!({ "path": "gbk.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert!(text.contains("系统找不到指定的路径。"));
    assert!(text.ends_with("[encoding: GBK (guess)]"));
}

#[tokio::test]
async fn utf16le_with_bom_is_readable_without_a_bom_character() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let mut bytes = Vec::from(b"\xFF\xFE");
    for unit in "中文\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    write_bytes(dir.path(), "utf16.txt", &bytes);

    let result = tool()
        .execute(&json!({ "path": "utf16.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert_eq!(
        text,
        format!("{}中文\n[encoding: UTF-16LE (bom)]", prefix(1))
    );
    assert!(!text.contains('\u{FEFF}'));
}

#[tokio::test]
async fn utf16_without_bom_is_rejected_with_an_encoding_hint() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let mut bytes = Vec::new();
    for unit in "hi".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    write_bytes(dir.path(), "utf16.txt", &bytes);

    let result = tool()
        .execute(&json!({ "path": "utf16.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("encoding"));
}

#[tokio::test]
async fn guessed_lossy_encoding_is_rejected_with_an_encoding_hint() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let mut bytes = b"\xB0\xA1".repeat(50);
    bytes.push(0xFF);
    write_bytes(dir.path(), "weird.txt", &bytes);

    let result = tool()
        .execute(&json!({ "path": "weird.txt" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("encoding"));
}

#[tokio::test]
async fn lossy_explicit_encoding_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write_bytes(dir.path(), "gbk.txt", b"\xFF\xFF");

    let result = tool()
        .execute(&json!({ "path": "gbk.txt", "encoding": "gbk" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("GBK"));
}

#[tokio::test]
async fn explicit_encoding_overrides_utf8_validation() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "a.txt", "中文\n");

    let result = tool()
        .execute(
            &json!({ "path": "a.txt", "encoding": "windows-1252" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let text = result.assistant_text();
    assert!(!text.contains("中文"));
    assert_eq!(
        text,
        format!(
            "{}ä¸\u{ad}æ\u{2013}\u{2021}\n[encoding: windows-1252]",
            prefix(1)
        )
    );
}

#[tokio::test]
async fn receipt_records_the_detected_encoding_and_bom_state() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let (tool, store) = tool_with_store();
    let (bytes, _, had_errors) = encoding_rs::GBK.encode("中文内容\n");
    assert!(!had_errors);
    let path = write_bytes(dir.path(), "gbk.txt", &bytes);

    tool.execute(&json!({ "path": "gbk.txt", "encoding": "gbk" }), &ctx)
        .await
        .unwrap();

    let key = assistant_tools::storage::ReadReceiptKey {
        session_id: ctx.session_id.clone(),
        path: path.display().to_string(),
        start_line: None,
        end_line: None,
    };
    let receipt = store.lookup(&key).await.expect("receipt recorded");
    assert_eq!(receipt.encoding, "GBK");
    assert!(!receipt.has_bom);
}
