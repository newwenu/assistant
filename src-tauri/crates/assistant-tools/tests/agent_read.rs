use std::path::Path;
use std::sync::Arc;

use assistant_tools::os::agent_read::{
    AgentReadTool, LINE_NUMBER_WIDTH, LINE_TRUNCATED_MARKER, MAX_LINE_CHARS,
};
use assistant_tools::storage::{
    FileToolResultStorage, InMemoryReadReceiptStore, OutputStream, PersistToolResult,
    ReadReceiptStore, ToolResultStorage,
};
use assistant_tools::{
    Tool, ToolAudience, ToolErrorKind, ToolTier, ToolUseContext, ValidationResult,
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
        schema["properties"].get("column").is_none(),
        "row-level paging belongs to I58 super_read, not to read"
    );
    assert!(
        schema["properties"].get("maxLineChars").is_none(),
        "read keeps MAX_LINE_CHARS as a hard cap, not a tunable window"
    );
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
}

#[test]
fn permission_intents_declare_read_on_the_resolved_path() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let tool = tool();
    let intents = tool
        .permission_intents(&json!({ "path": "a.txt" }), &ctx)
        .expect("permission intents");
    assert_eq!(intents.len(), 1, "read 必须声明 1 条 intent");
    assert_eq!(intents[0].action, "read");
    assert_eq!(
        intents[0].resources,
        vec![assistant_tools::os::path::canonicalize_lenient(dir.path())
            .expect("canonical root")
            .join("a.txt")
            .display()
            .to_string()],
        "意图必须是解析后的绝对路径，策略按绝对路径匹配"
    );
}

#[test]
fn permission_intents_use_read_tool_output_for_the_output_namespace() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let intents = tool()
        .permission_intents(
            &json!({ "path": output.path().join("run.log").display().to_string() }),
            &ctx,
        )
        .expect("permission intents");
    assert_eq!(intents.len(), 1);
    assert_eq!(
        intents[0].action, "readToolOutput",
        "输出域必须走更严的独立动作"
    );
    assert_eq!(
        intents[0].resources,
        vec![output.path().display().to_string()],
        "resource 取命名空间目录：一次授权覆盖本会话全部输出"
    );

    // 工作区内仍走 `read`，resource 是具体文件。
    let inside = tool()
        .permission_intents(&json!({ "path": "a.txt" }), &ctx)
        .expect("workspace intent");
    assert_eq!(inside[0].action, "read");
}

#[tokio::test]
async fn execute_reads_a_file_in_the_output_namespace() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"line one\nline two\n").unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let result = tool()
        .execute(
            &json!({ "path": output.path().join("run.log").display().to_string() }),
            &ctx,
        )
        .await
        .expect("execute must not fail");
    assert!(
        result.ok,
        "output file read must succeed: {:?}",
        result.error
    );
}

#[test]
fn validate_input_rejects_a_foreign_output_namespace() {
    let workspace = tempfile::tempdir().unwrap();
    let own_output = tempfile::tempdir().unwrap();
    let foreign_output = tempfile::tempdir().unwrap();
    std::fs::write(foreign_output.path().join("run.log"), b"log").unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(own_output.path().to_path_buf());

    let result = tool().validate_input(
        &json!({ "path": foreign_output.path().join("run.log").display().to_string() }),
        &ctx,
    );
    assert!(
        !result.ok,
        "其它会话的输出目录既不在工作区也不在本会话输出域，必须在边界处拒绝"
    );
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn preview_for_approval_describes_the_output_window() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let path = output.path().join("build.log");
    std::fs::write(&path, "line one\nline two\n").unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let preview = tool()
        .preview_for_approval(
            &json!({ "path": path.display().to_string(), "offset": 10, "limit": 5 }),
            &ctx,
        )
        .await
        .expect("output-domain read must carry a magnitude summary");
    assert_eq!(
        preview.summary,
        "读本会话输出 build.log · 第 10–14 行 · 18 B"
    );
    assert!(!preview.truncated, "行窗口已知，不存在扫描上限");
    assert!(!preview.estimated, "字节数来自 stat，不是估算");
}

#[tokio::test]
async fn preview_for_approval_is_absent_inside_the_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    write(workspace.path(), "a.txt", "hello\n");

    let preview = tool()
        .preview_for_approval(
            &json!({ "path": "a.txt" }),
            &context(workspace.path(), None),
        )
        .await;
    assert!(
        preview.is_none(),
        "工作区内读取由 `read` 的默认 Allow 覆盖，不该产出审批量级摘要"
    );
}

#[tokio::test]
async fn spilled_output_is_readable_and_pageable_to_the_end() {
    let workspace = tempfile::tempdir().unwrap();
    let storage_root = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(storage_root.path());
    let session = "conv-1";

    let mut sink = storage
        .open_sink(PersistToolResult {
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
            session_id: Some(session.to_string()),
            content: String::new(),
        })
        .await
        .expect("open sink");

    let mut chunk = String::new();
    for line in 1..=100_000u32 {
        chunk.push_str(&format!("line {}\n", line));
        if chunk.len() >= 8192 {
            sink.write(OutputStream::Stdout, chunk.as_bytes())
                .expect("sink write must not fail");
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        sink.write(OutputStream::Stdout, chunk.as_bytes())
            .expect("sink write must not fail");
    }
    let spilled = sink.finish().expect("finish must succeed");
    let reference = spilled
        .reference
        .expect("non-empty output must leave a file behind");
    assert_eq!(spilled.output_lines, 100_000);

    let namespace = storage
        .namespace_root(Some(session))
        .expect("file storage provides a read-back root");
    assert!(
        Path::new(&reference.path).starts_with(&namespace),
        "输出必须落在本会话命名空间目录内: {}",
        reference.path
    );

    let mut ctx = context(workspace.path(), Some(session));
    ctx.tool_output_root = Some(namespace);
    let read_tool = tool();

    let head = read_tool
        .execute(&json!({ "path": reference.path, "limit": 10 }), &ctx)
        .await
        .expect("execute must not fail");
    assert!(head.ok, "输出域文件必须可读: {:?}", head.error);
    let head_text = head.result_for_assistant.unwrap_or_default();
    assert!(
        head_text.contains("line 1\n"),
        "unexpected head: {head_text}"
    );
    assert!(
        !head_text.contains("line 11\n"),
        "limit 必须截住窗口: {head_text}"
    );

    let tail = read_tool
        .execute(
            &json!({ "path": reference.path, "offset": 99_991, "limit": 10 }),
            &ctx,
        )
        .await
        .expect("execute must not fail");
    assert!(tail.ok, "输出域文件必须可分页: {:?}", tail.error);
    let tail_text = tail.result_for_assistant.unwrap_or_default();
    assert!(
        tail_text.contains("line 100000\n"),
        "必须能分页读到末尾: {tail_text}"
    );
}

#[test]
fn permission_intents_report_errors_instead_of_empty() {
    let tool = tool();
    let error = tool
        .permission_intents(&json!({ "path": "a.txt" }), &ToolUseContext::default())
        .expect_err("无工作区基准时必须报错，不能静默返回空意图");
    assert!(
        error.contains("cannot be resolved"),
        "unexpected error: {error}"
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
        json!({ "path": "a.txt", "limit": 0 }),
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
    // 模型把数字写成字符串是常见偏差：宽松解析接受，由管线归一化后执行。
    assert_eq!(
        tool.validate_input(
            &json!({ "path": "a.txt", "offset": "1", "limit": "10" }),
            &ctx
        ),
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
    assert!(text.contains(&format!("{LINE_TRUNCATED_MARKER}\n")));
    assert!(text.contains(&"x".repeat(MAX_LINE_CHARS)));
    assert!(!text.contains(&long_line));
    assert!(
        text.ends_with(&format!(
            "[long-lines: 1 lines cut at {} chars]",
            MAX_LINE_CHARS
        )),
        "长行 footer 必须独占最后一行: {text:?}"
    );
}

/// G6：`truncatedLines` 恒在、类型稳定、不超过 `returnedLines`，且与 `truncated` 正交
/// （单行超宽时窗口取完了行，但行本身被切：`truncated = false` 而 `truncatedLines > 0`）。
#[tokio::test]
async fn truncated_lines_is_reported_and_orthogonal_to_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let huge = "q".repeat(4 * 1024 * 1024);
    write(dir.path(), "one-line.txt", &huge);

    let result = tool()
        .execute(&json!({ "path": "one-line.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncatedLines"], json!(1));
    assert_eq!(data["truncated"], json!(false));
    assert_eq!(data["endLine"], json!(1));
}

/// G6 的另一半：普通文件 `truncatedLines = 0`，字段仍在（键恒在，不是省略）。
#[tokio::test]
async fn truncated_lines_is_zero_for_ordinary_files() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    write(dir.path(), "plain.txt", "alpha\nbeta\n");

    let result = tool()
        .execute(&json!({ "path": "plain.txt" }), &ctx)
        .await
        .unwrap();

    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncatedLines"], json!(0));
    assert!(!result.assistant_text().contains("[long-lines"));
}

/// G7：`[long-lines]` 在 `[encoding]` 之前、各占一行，`n` 与 `data.truncatedLines` 同源。
#[tokio::test]
async fn long_lines_footer_precedes_the_encoding_footer() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let mut content = String::from("\u{FEFF}");
    content.push_str(&"w".repeat(MAX_LINE_CHARS + 10));
    content.push('\n');
    write(dir.path(), "bom-long.txt", &content);

    let result = tool()
        .execute(&json!({ "path": "bom-long.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncatedLines"], json!(1));

    let text = result.assistant_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.last(),
        Some(&"[encoding: UTF-8 (bom)]"),
        "编码 footer 必须在最后: {text:?}"
    );
    assert_eq!(
        lines[lines.len() - 2],
        format!("[long-lines: 1 lines cut at {} chars]", MAX_LINE_CHARS)
    );
}

/// G7：`truncatedLines` 与 `returnedLines` 的包含关系（多行被切时计数逐行累加）。
#[tokio::test]
async fn truncated_lines_counts_every_cut_line() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let wide = "v".repeat(MAX_LINE_CHARS + 5);
    let content = format!("{}\n{}\nshort\n", wide, wide);
    write(dir.path(), "two-long.txt", &content);

    let result = tool()
        .execute(&json!({ "path": "two-long.txt" }), &ctx)
        .await
        .unwrap();

    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncatedLines"], json!(2));
    assert_eq!(data["endLine"], json!(3));
    assert!(result
        .assistant_text()
        .contains("[long-lines: 2 lines cut at 2000 chars]"));
}

/// G3 的文案面：窗口**未满**但预算耗尽时走 S3（不是「more content follows」式的含糊说法）。
///
/// 两个构造条件缺一不可：① 每行预算够大，使字节预算在 2000 行之前耗尽（窗口未满）；
/// ② 剩余内容超过 8 MiB 宽限，否则扫描会读到 EOF 并给出精确总数（那是 S1）。
/// 20,000 行 × 600 字节：预算在第 ~1727 行收口，剩余 ≈ 11 MB > 宽限；行本身不超宽，
/// 因此 `truncatedLines = 0`、无 `[long-lines]`，可见 S3 与长行截断是两件事。
#[tokio::test]
async fn a_budget_stop_before_the_line_cap_is_reported_as_a_scan_budget_stop() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let line = format!("{}\n", "y".repeat(600));
    let content = line.repeat(20_000);
    write(dir.path(), "budget.txt", &content);

    let result = tool()
        .execute(&json!({ "path": "budget.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncated"], json!(true));
    assert_eq!(data["totalLines"], Value::Null);
    assert_eq!(data["truncatedLines"], json!(0));

    let text = result.assistant_text();
    assert!(
        text.contains("scan budget exhausted after"),
        "窗口未满的停止必须归因到扫描预算: {text:?}"
    );
    assert!(text.contains("total unknown"));
    assert!(!text.contains("[long-lines"), "没有行被切就没有长行 footer");
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
async fn repeated_reads_return_full_content() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some("session-1"));
    write(dir.path(), "a.txt", "alpha\nbeta\n");

    let tool = tool();
    let first = tool
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(first.assistant_text().contains("alpha"));

    let second = tool
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(second.ok);
    assert_eq!(second.assistant_text(), first.assistant_text());
    let data = second.data.as_ref().unwrap();
    assert_eq!(data["startLine"], json!(1));
    assert_eq!(data["totalLines"], json!(2));
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

    assert!(refreshed.assistant_text().contains("beta"));
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
        // `read` 记录的是归一后的路径（macOS 上 `/var` → `/private/var`）。
        path: assistant_tools::os::path::canonicalize_lenient(&path)
            .expect("canonical path")
            .display()
            .to_string(),
    };
    let receipt = store.lookup(&key).await.expect("receipt recorded");
    assert_eq!(receipt.encoding, "GBK");
    assert!(!receipt.has_bom);
    assert_eq!(receipt.truncated_lines, 0, "完整读取不得留下切行标记");
    assert_eq!(
        receipt.line_window_max_chars, 2000,
        "A 侧的每行宽度是常量，供 edit 闸门文案归因"
    );
}

/// 回执里的 `truncated_lines` 与 `data.truncatedLines` 同源（I57 §3.5 的唯一写入点）。
#[tokio::test]
async fn a_receipt_records_the_cut_line_count_for_a_long_line_read() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let (tool, store) = tool_with_store();
    let path = write(dir.path(), "wide.txt", &format!("{}\n", "z".repeat(5000)));

    let result = tool
        .execute(&json!({ "path": "wide.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.data.as_ref().unwrap()["truncatedLines"], json!(1));

    let key = assistant_tools::storage::ReadReceiptKey {
        session_id: ctx.session_id.clone(),
        path: assistant_tools::os::path::canonicalize_lenient(&path)
            .expect("canonical path")
            .display()
            .to_string(),
    };
    let receipt = store.lookup(&key).await.expect("receipt recorded");
    assert_eq!(receipt.truncated_lines, 1);
    assert_eq!(receipt.line_window_max_chars, 2000);
}

/// 扫描块大小，与 `agent_read::window::CHUNK_BYTES` 同值。
const CHUNK_BYTES: usize = 256 * 1024;

/// 构造「恰好把下一个多字节字符的第一个字节留在块边界之前」的 ASCII 填充。
///
/// `ab\n` 每行 3 字节，`(CHUNK_BYTES - 1) / 3` 行正好是 `CHUNK_BYTES - 1` 字节，
/// 因此随后的多字节字符必然跨块。
fn filler_ending_one_byte_before_the_chunk_boundary() -> String {
    let lines = (CHUNK_BYTES - 1) / 3;
    let filler = "ab\n".repeat(lines);
    assert_eq!(filler.len(), CHUNK_BYTES - 1);
    filler
}

#[tokio::test]
async fn a_gbk_character_split_across_chunks_is_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let filler = filler_ending_one_byte_before_the_chunk_boundary();
    let (gbk, _, had_errors) = encoding_rs::GBK.encode("中文\n");
    assert!(!had_errors);
    let mut bytes = filler.clone().into_bytes();
    bytes.extend_from_slice(&gbk);
    write_bytes(dir.path(), "split-gbk.txt", &bytes);

    let last_line = filler.matches('\n').count() + 1;
    let result = tool()
        .execute(
            &json!({ "path": "split-gbk.txt", "encoding": "gbk", "offset": last_line, "limit": 1 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let text = result.assistant_text();
    assert_eq!(text, format!("{}中文\n[encoding: GBK]", prefix(last_line)));
}

#[tokio::test]
async fn a_utf8_character_split_across_chunks_is_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let filler = filler_ending_one_byte_before_the_chunk_boundary();
    let mut bytes = filler.clone().into_bytes();
    bytes.extend_from_slice("中文\n".as_bytes());
    write_bytes(dir.path(), "split-utf8.txt", &bytes);

    let last_line = filler.matches('\n').count() + 1;
    let result = tool()
        .execute(
            &json!({ "path": "split-utf8.txt", "offset": last_line, "limit": 1 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let text = result.assistant_text();
    assert_eq!(text, format!("{}中文\n", prefix(last_line)));
}

#[tokio::test]
async fn files_above_the_old_whole_file_limit_are_readable() {
    const LINES: usize = 170_000;
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let line = format!("{}\n", "z".repeat(100));
    let mut content = String::with_capacity(LINES * line.len());
    for _ in 0..LINES {
        content.push_str(&line);
    }
    assert!(content.len() > 16 * 1024 * 1024);
    write(dir.path(), "big.log", &content);

    let head = tool()
        .execute(&json!({ "path": "big.log" }), &ctx)
        .await
        .unwrap();

    assert!(head.ok, "{:?}", head.error);
    let data = head.data.as_ref().unwrap();
    assert_eq!(data["truncated"], json!(true));
    assert_eq!(data["totalLines"], Value::Null);
    let text = head.assistant_text();
    assert!(text.contains("showing 2000 lines"));
    assert!(text.contains("total unknown"));

    let tail = tool()
        .execute(
            &json!({ "path": "big.log", "offset": LINES - 3, "limit": 3 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(tail.ok, "{:?}", tail.error);
    let data = tail.data.as_ref().unwrap();
    assert_eq!(data["totalLines"], json!(LINES));
    assert_eq!(data["startLine"], json!(LINES - 3));
    assert_eq!(data["endLine"], json!(LINES - 1));
    let text = tail.assistant_text();
    assert_eq!(text.lines().count(), 3);
    assert!(text.contains(&format!("{}\t{}", LINES - 1, "z".repeat(100))));
}

#[tokio::test]
async fn a_single_line_longer_than_the_output_budget_is_cut_without_loading_it() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let huge = "q".repeat(4 * 1024 * 1024);
    write(dir.path(), "one-line.txt", &huge);

    let result = tool()
        .execute(&json!({ "path": "one-line.txt" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalLines"], json!(1));
    assert_eq!(data["truncated"], json!(false));
    let text = result.assistant_text();
    assert!(text.contains(&"q".repeat(MAX_LINE_CHARS)));
    assert!(text.contains(LINE_TRUNCATED_MARKER));
    assert!(!text.contains(&"q".repeat(MAX_LINE_CHARS + 1)));
}

#[tokio::test]
async fn a_utf8_file_that_turns_invalid_later_falls_back_to_a_guess() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    // 头部 8KB 全 ASCII（通过 UTF-8 校验），之后整篇是 GBK 正文：UTF-8 乐观路径必须在
    // 第一个非法块处回退，并按头部 1MB 样本重猜编码后从头重读。
    let head = "ab\n".repeat((8 * 1024) / 3 + 1);
    assert!(head.len() > 8 * 1024);
    let body_lines = 30_000;
    let body_text = "中文测试。\n".repeat(body_lines);
    let (body, _, had_errors) = encoding_rs::GBK.encode(&body_text);
    assert!(!had_errors);
    let mut bytes = head.clone().into_bytes();
    bytes.extend_from_slice(&body);
    write_bytes(dir.path(), "late-invalid.txt", &bytes);

    let head_lines = head.matches('\n').count();
    let last_line = head_lines + body_lines;
    let result = tool()
        .execute(
            &json!({ "path": "late-invalid.txt", "offset": last_line, "limit": 1 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok, "{:?}", result.error);
    let text = result.assistant_text();
    assert!(text.contains("中文测试。"));
    assert!(text.ends_with("(guess)]"));
}

/// A1：`tool-output://<name>` 读到的内容与绝对路径读逐字节一致（I73 §7）。
#[tokio::test]
async fn read_accepts_a_tool_output_reference() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    write(output.path(), "run.log", "line one\nline two\n");

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let by_ref = tool()
        .execute(&json!({ "path": "tool-output://run.log" }), &ctx)
        .await
        .unwrap();
    let by_path = tool()
        .execute(
            &json!({ "path": output.path().join("run.log").display().to_string() }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(by_ref.ok, "{:?}", by_ref.error);
    assert_eq!(
        by_ref.result_for_assistant, by_path.result_for_assistant,
        "引用读取必须与绝对路径读取逐字节一致"
    );
    assert_eq!(by_ref.data, by_path.data);
}

/// A3：两种写法命中同一授权键，因此不会二次弹框（I73 §7）。
///
/// 授权键由 `(action, resource)` 派生，断言两者相同即等价于「同一键」。
#[test]
fn reference_and_absolute_path_share_one_authorization_key() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let by_ref = tool()
        .permission_intents(&json!({ "path": "tool-output://run.log" }), &ctx)
        .expect("ref intents");
    let by_path = tool()
        .permission_intents(
            &json!({ "path": output.path().join("run.log").display().to_string() }),
            &ctx,
        )
        .expect("absolute intents");

    assert_eq!(by_ref, by_path, "同一目标必须派生同一授权键");
}

/// A7：没有输出域时引用读取报业务错误，不回退工作区（I73 §7）。
#[tokio::test]
async fn a_reference_without_an_output_root_does_not_fall_back_to_the_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    write(workspace.path(), "run.log", "workspace copy\n");
    let ctx = context(workspace.path(), Some("conv-1"));

    let result = tool()
        .execute(&json!({ "path": "tool-output://run.log" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    let error = result.error.unwrap_or_default();
    assert!(
        error.contains("no tool output root"),
        "unexpected error: {error}"
    );
}

/// A11：`resolve_readable(model_ref(path))` 回到逐字节相同的绝对路径（I73 §7）。
#[test]
fn a_tool_output_reference_round_trips_to_the_persisted_path() {
    use assistant_tools::os::path::{parse_ref, tool_output_ref, ReadableScope};

    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let persisted = write(output.path(), "run.log", "log\n");

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let reference = tool_output_ref("run.log");
    let resolved = parse_ref(&reference)
        .expect("parse ref")
        .resolve_readable(&ctx)
        .expect("resolve ref");

    assert_eq!(
        resolved.path,
        assistant_tools::os::path::canonicalize_lenient(&persisted).expect("canonical path"),
        "回读必须命中落盘时的那一份"
    );
    assert_eq!(resolved.scope, ReadableScope::ToolOutput);
}

/// A22：域根以 verbatim 形态注入时仍能读回域内文件（I73 §3.9 / N17）。
#[cfg(windows)]
#[tokio::test]
async fn a_verbatim_output_root_still_reads_its_own_files() {
    let workspace = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    write(output.path(), "run.log", "verbatim\n");

    let verbatim = std::path::PathBuf::from(format!(r"\\?\{}", output.path().display()));
    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_output_root = Some(verbatim.clone());

    let absolute = verbatim.join("run.log").display().to_string();
    for path in ["tool-output://run.log".to_string(), absolute.clone()] {
        let result = tool()
            .execute(&json!({ "path": path }), &ctx)
            .await
            .unwrap();
        assert!(result.ok, "path '{}' must read: {:?}", path, result.error);
    }

    let intents = tool()
        .permission_intents(&json!({ "path": absolute }), &ctx)
        .expect("intents");
    assert_eq!(intents[0].action, "readToolOutput");
}

/// A17：scratch 越界与形态拒绝；`scratch://` 与 `scratch:///` 指会话临时区根本身，
/// 读取目录报「is a directory」而不是越界（I73 §7 A17 / N15）。
#[tokio::test]
async fn scratch_references_reject_escapes_and_bare_forms_mean_the_root() {
    let workspace = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "inside.txt", "inside\n");

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_scratch_root = Some(scratch.path().to_path_buf());

    for path in ["scratch://../x", "scratch://a/../../x", r"scratch://C:\x"] {
        let result = tool()
            .execute(&json!({ "path": path }), &ctx)
            .await
            .unwrap();
        assert!(!result.ok, "{path} must be rejected");
        let error = result.error.unwrap_or_default();
        assert!(error.contains("invalid path reference"), "{path}: {error}");
    }

    // 根形态合法：落到目录本身，失败原因是「读的是目录」而非「越界」。
    for root in ["scratch://", "scratch:///"] {
        let result = tool()
            .execute(&json!({ "path": root }), &ctx)
            .await
            .unwrap();
        assert!(!result.ok, "{root} must fail as a directory");
        let error = result.error.unwrap_or_default();
        assert!(error.contains("is a directory"), "{root}: {error}");
        assert!(!error.contains("escapes"), "{root}: {error}");
    }

    // 域内文件：审批意图是 `readScratch`，一次授权覆盖本会话全部临时区。
    let intents = tool()
        .permission_intents(&json!({ "path": "scratch://inside.txt" }), &ctx)
        .expect("intents");
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].action, "readScratch");
    assert_eq!(
        intents[0].resources,
        vec![scratch.path().display().to_string()]
    );
}

/// A24：引用类路径错误的回显口径——只含调用方原串，不含域根物理路径
/// （含用户名与会话哈希）（I73 §3.10 D1 / D2 / N18）。
#[tokio::test]
async fn reference_errors_echo_the_raw_reference_not_the_domain_root() {
    let workspace = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_scratch_root = Some(scratch.path().to_path_buf());
    ctx.tool_output_root = Some(output.path().to_path_buf());

    let scratch_root = scratch.path().display().to_string();
    let output_root = output.path().display().to_string();

    for path in ["scratch://missing.txt", "tool-output://missing.log"] {
        let result = tool()
            .execute(&json!({ "path": path }), &ctx)
            .await
            .unwrap();
        assert!(!result.ok, "{path} must fail");
        let error = result.error.unwrap_or_default();
        assert!(
            error.contains(path),
            "{path} must echo the raw reference: {error}"
        );
        assert!(
            !error.contains(&scratch_root) && !error.contains(&output_root),
            "{path} must not leak the domain root: {error}"
        );
    }

    // 读临时区根本身：失败原因是「读的是目录」，同样只回显原串。
    let result = tool()
        .execute(&json!({ "path": "scratch://" }), &ctx)
        .await
        .unwrap();
    assert!(
        !result.ok,
        "reading the scratch root must fail as a directory"
    );
    let error = result.error.unwrap_or_default();
    assert!(error.contains("is a directory"), "{error}");
    assert!(error.contains("scratch://"), "{error}");
    assert!(
        !error.contains(&scratch_root),
        "directory error must not leak the domain root: {error}"
    );
}

/// A25：解析层的失败文案同口径——引用命中受保护目录时也只回显原串（I73 §3.10 D2 / N18）。
#[tokio::test]
async fn parse_layer_errors_echo_the_raw_reference_too() {
    let workspace = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();

    let mut ctx = context(workspace.path(), Some("conv-1"));
    ctx.tool_scratch_root = Some(scratch.path().to_path_buf());

    let path = "scratch://.ssh/id_rsa";
    let result = tool()
        .execute(&json!({ "path": path }), &ctx)
        .await
        .unwrap();
    assert!(!result.ok, "{path} must be rejected");
    let error = result.error.unwrap_or_default();
    assert!(error.contains(path), "must echo the raw reference: {error}");
    assert!(
        error.contains("credential directory"),
        "must still report why it was rejected: {error}"
    );
    assert!(
        !error.contains(&scratch.path().display().to_string()),
        "must not leak the domain root: {error}"
    );
}
