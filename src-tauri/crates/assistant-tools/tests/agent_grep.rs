use std::path::Path;
use std::time::Duration;

use assistant_tools::os::agent_grep::{
    AgentGrepTool, MAX_CONTEXT, MAX_FILESIZE, MAX_MATCHES, MAX_OUTPUT_LINES,
};
use assistant_tools::{Tool, ToolAudience, ToolErrorKind, ToolTier, ToolUseContext};
use serde_json::json;

fn context(root: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: None,
        tool_call_id: Some("call-1".to_string()),
    }
}

fn tool() -> AgentGrepTool {
    AgentGrepTool::new()
}

fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dir");
    }
    std::fs::write(path, content).expect("write fixture");
}

fn write_bytes(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dir");
    }
    std::fs::write(path, content).expect("write fixture");
}

fn gbk(text: &str) -> Vec<u8> {
    let (bytes, _, had_errors) = encoding_rs::GBK.encode(text);
    assert!(!had_errors, "fixture must be lossless");
    bytes.into_owned()
}

#[test]
fn schema_declares_pattern_path_include_and_context() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["pattern"]));
    assert!(schema["properties"]["pattern"].is_object());
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["include"].is_object());
    assert!(schema["properties"]["context"].is_object());
    assert!(schema["properties"]["encoding"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_readonly_contract() {
    let tool = tool();
    assert_eq!(tool.id(), "grep");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(tool.is_concurrency_safe(None));
    assert_eq!(
        tool.permission_intents(&json!({ "pattern": "hit" }), &ToolUseContext::default()),
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
        json!({ "pattern": 42 }),
        json!({ "pattern": "  " }),
        json!({ "pattern": "hit", "path": 42 }),
        json!({ "pattern": "hit", "include": 42 }),
        json!({ "pattern": "hit", "context": -1 }),
        json!({ "pattern": "hit", "context": "2" }),
        json!({ "pattern": "hit", "context": MAX_CONTEXT + 1 }),
        json!({ "pattern": "hit(", "path": "." }),
        json!({ "pattern": "hit", "include": "*.{rs", "path": "." }),
        json!({ "pattern": "hit", "encoding": 42 }),
        json!({ "pattern": "hit", "encoding": "not-an-encoding", "path": "." }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert!(tool.validate_input(&json!({ "pattern": "hit" }), &ctx).ok);
    assert!(
        tool.validate_input(
            &json!({ "pattern": "hit", "path": ".", "include": "*.rs", "context": 2 }),
            &ctx
        )
        .ok
    );
    assert!(
        tool.validate_input(
            &json!({ "pattern": "hit", "path": ".", "encoding": "gbk" }),
            &ctx
        )
        .ok
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());

    let result = tool().validate_input(
        &json!({ "pattern": "hit", "path": other.path().display().to_string() }),
        &ctx,
    );
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn matches_report_path_line_number_and_text() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "src/a.txt", "alpha\nhit\nbeta\n");

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "src/a.txt:2:hit");
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(data["truncated"], json!(false));
    assert_eq!(data["timedOut"], json!(false));
    assert_eq!(data["matches"], json!(["src/a.txt:2:hit"]));
}

#[tokio::test]
async fn context_adds_surrounding_lines_and_separates_groups() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(
        dir.path(),
        "log.txt",
        "one\ntwo\nhit\nfour\nfive\nsix\nseven\neight\nhit\nten\n",
    );

    let with_context = tool()
        .execute(
            &json!({ "pattern": "hit", "path": ".", "context": 1 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(with_context.ok);
    assert_eq!(
        with_context.assistant_text(),
        "log.txt-2-two\nlog.txt:3:hit\nlog.txt-4-four\n--\nlog.txt-8-eight\nlog.txt:9:hit\nlog.txt-10-ten"
    );

    let without_context = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();
    assert!(without_context.ok);
    // `context = 0` 时没有 context 窗口，`grep-searcher` 不会给出 `context_break`，
    // 与 rg 一致：连续匹配行不加分隔符（行号本身已标出跳行）。
    assert_eq!(
        without_context.assistant_text(),
        "log.txt:3:hit\nlog.txt:9:hit"
    );
}

#[tokio::test]
async fn include_filters_candidate_files() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "src/a.rs", "needle\n");
    write_file(dir.path(), "docs/b.md", "needle\n");

    let result = tool()
        .execute(
            &json!({ "pattern": "needle", "path": ".", "include": "*.rs" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "src/a.rs:1:needle");
}

#[tokio::test]
async fn include_with_a_slash_only_matches_that_depth() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "src/a.rs", "needle\n");
    write_file(dir.path(), "src/deep/b.rs", "needle\n");

    let direct = tool()
        .execute(
            &json!({ "pattern": "needle", "path": ".", "include": "src/*.rs" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(direct.ok);
    assert_eq!(
        direct.assistant_text(),
        "src/a.rs:1:needle",
        "a pattern containing '/' must not recurse"
    );

    // 不含 `/` 的 pattern 自动前置 `**/`：`*.rs` 与 `**/*.rs` 等价，两个层级都命中。
    let recursive = tool()
        .execute(
            &json!({ "pattern": "needle", "path": ".", "include": "*.rs" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(recursive.ok);
    assert_eq!(
        recursive.assistant_text(),
        "src/a.rs:1:needle\nsrc/deep/b.rs:1:needle"
    );
}

#[tokio::test]
async fn the_pattern_is_compiled_verbatim_including_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "  hit\nhit\n");

    let indented = tool()
        .execute(&json!({ "pattern": "  hit", "path": "." }), &ctx)
        .await
        .unwrap();
    assert!(indented.ok);
    assert_eq!(
        indented.assistant_text(),
        "a.txt:1:  hit",
        "leading whitespace in the pattern must stay significant"
    );
}

#[tokio::test]
async fn context_above_the_limit_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "hit\n");

    let result = tool()
        .execute(
            &json!({ "pattern": "hit", "path": ".", "context": MAX_CONTEXT + 1 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    let message = result.error.unwrap_or_default();
    assert!(message.contains("context"));
    assert!(message.contains(&MAX_CONTEXT.to_string()));
}

#[tokio::test]
async fn context_lines_share_the_output_line_budget() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    // 60 个命中、每个间隔 11 行：`context = 10` 时相邻窗口全部连成一片，行数远超 300。
    let mut body = String::new();
    for _ in 0..60 {
        body.push_str("hit\n");
        for _ in 0..11 {
            body.push_str("pad\n");
        }
    }
    write_file(dir.path(), "big.txt", &body);

    let result = tool()
        .execute(
            &json!({ "pattern": "hit", "path": ".", "context": MAX_CONTEXT }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["truncated"], json!(true));
    let lines = data["matches"].as_array().unwrap();
    assert_eq!(
        lines.len(),
        MAX_OUTPUT_LINES,
        "context lines must be capped by the shared line budget"
    );
    assert!(result.assistant_text().contains("output line limit"));
}

#[tokio::test]
async fn invalid_utf8_hit_lines_are_flagged_as_lossy() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_bytes(dir.path(), "zh.txt", &gbk("中文 hit\n"));

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(data["lossyLines"], json!(1));
    assert!(result.assistant_text().contains("[lossy: 1 line(s)]"));
}

#[tokio::test]
async fn an_explicit_encoding_skips_the_probe() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_bytes(dir.path(), "zh.txt", &gbk("中文内容\n"));

    let result = tool()
        .execute(
            &json!({ "pattern": "no_such_token", "path": ".", "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["nonUtf8Probe"], json!(null));
    assert_eq!(
        result.assistant_text(),
        "(no matches)",
        "an explicit encoding already transcoded the scope, so no probe footer"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn unreadable_files_are_counted_and_do_not_abort_the_search() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "hit\n");
    write_file(dir.path(), "locked.txt", "hit\n");
    // 独占句柄（share_mode 0）让 searcher 打开失败，模拟读取错误。
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.path().join("locked.txt"))
        .unwrap();

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();
    drop(handle);

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["unreadableFiles"], json!(1));
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(result.assistant_text(), "a.txt:1:hit\n[skipped: 1 file(s)]");
}

#[tokio::test]
async fn matches_are_capped_at_one_hundred_with_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let body: String = (0..(MAX_MATCHES + 50))
        .map(|_| "hit\n")
        .collect::<Vec<_>>()
        .concat();
    write_file(dir.path(), "many.txt", &body);

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(MAX_MATCHES));
    assert_eq!(data["truncated"], json!(true));
    let matches = data["matches"].as_array().unwrap();
    assert_eq!(matches.len(), MAX_MATCHES);
    let text = result.assistant_text();
    assert!(text.contains(&format!("showing the first {}", MAX_MATCHES)));
    assert!(text.contains("narrow"));
}

#[tokio::test]
async fn invalid_regex_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());

    let result = tool()
        .execute(&json!({ "pattern": "hit(", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("invalid regex"));
}

#[tokio::test]
async fn an_empty_result_is_reported_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "alpha\n");

    let result = tool()
        .execute(&json!({ "pattern": "missing", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "(no matches)");
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(0));
}

#[tokio::test]
async fn toolbox_injected_action_field_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "hit\n");

    let result = tool()
        .execute(
            &json!({ "action": "grep", "pattern": "hit", "path": "." }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "a.txt:1:hit");
}

#[tokio::test]
async fn defaults_the_base_directory_to_the_workspace_root() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "root.txt", "hit\n");

    let result = tool()
        .execute(&json!({ "pattern": "hit" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "root.txt:1:hit");
}

#[tokio::test]
async fn a_missing_workspace_root_is_a_business_error() {
    let ctx = ToolUseContext::default();
    let result = tool()
        .execute(&json!({ "pattern": "hit" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("path"));
}

#[tokio::test]
async fn the_internal_deadline_stops_the_walk_and_keeps_partial_results() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let body: String = std::iter::repeat_with(|| "padding padding padding\n")
        .take(200)
        .collect();
    for index in 0..800 {
        write_file(dir.path(), &format!("bulk/{:04}.txt", index), &body);
    }

    let result = AgentGrepTool::with_budget(Duration::from_millis(501))
        .execute(
            &json!({ "pattern": "zzz_no_such_token", "path": "." }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["timedOut"], json!(true));
    assert_eq!(data["totalMatches"], json!(0));
    assert!(result.assistant_text().contains("time budget"));
    assert!(result.assistant_text().contains("may be incomplete"));
}

#[tokio::test]
async fn an_explicit_encoding_makes_non_utf8_files_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_bytes(dir.path(), "zh.txt", &gbk("第一行\n目标行\n第三行\n"));

    let without = tool()
        .execute(&json!({ "pattern": "目标行", "path": "." }), &ctx)
        .await
        .unwrap();
    assert!(without.ok);
    assert_eq!(without.data.as_ref().unwrap()["totalMatches"], json!(0));

    let with = tool()
        .execute(
            &json!({ "pattern": "目标行", "path": ".", "encoding": "gbk" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(with.ok, "{:?}", with.error);
    let data = with.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(data["lossyLines"], json!(0));
    assert_eq!(with.assistant_text(), "zh.txt:2:目标行");
}

#[tokio::test]
async fn zero_matches_probe_reports_non_utf8_files() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_bytes(dir.path(), "zh.txt", &gbk("中文内容\n"));
    write_file(dir.path(), "en.txt", "plain ascii\n");
    write_bytes(dir.path(), "bin.dat", b"\x00\x01binary");

    let result = tool()
        .execute(&json!({ "pattern": "no_such_token", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["nonUtf8Probe"]["files"], json!(1));
    assert_eq!(data["nonUtf8Probe"]["truncated"], json!(false));
    let text = result.assistant_text();
    assert!(text.contains("(no matches)"));
    assert!(text.contains("[non-utf8: 1 file(s)"));
}

#[tokio::test]
async fn a_utf8_only_scope_reports_no_non_utf8_footer() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "alpha\n");
    write_bytes(dir.path(), "bom.txt", "\u{feff}bom body\n".as_bytes());

    let result = tool()
        .execute(&json!({ "pattern": "no_such_token", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["nonUtf8Probe"]["files"], json!(0));
    assert_eq!(result.assistant_text(), "(no matches)");
}

#[tokio::test]
async fn hits_skip_the_probe_entirely() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "hit\n");
    write_bytes(dir.path(), "zh.txt", &gbk("中文内容\n"));

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.data.as_ref().unwrap()["nonUtf8Probe"], json!(null));
}

#[tokio::test]
async fn binary_files_are_skipped_without_garbage_lines() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_bytes(dir.path(), "bin.dat", b"\x00\x01hit\x00tail");
    write_file(dir.path(), "a.txt", "hit\n");

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["binarySkipped"], json!(1));
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(
        result.assistant_text(),
        "a.txt:1:hit\n[skipped: 1 file(s)]",
        "the skipped footer reports the binary file without touching the hit lines"
    );
}

#[tokio::test]
async fn a_binary_file_is_discarded_whole_even_after_a_hit() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    // 命中出现在 NUL 之前：逐行即时输出会把这一行留下，按文件缓冲才能整文件丢弃。
    write_bytes(dir.path(), "bin.dat", b"hit\n\x00tail");
    write_file(dir.path(), "a.txt", "hit\n");

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["binarySkipped"], json!(1));
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(result.assistant_text(), "a.txt:1:hit\n[skipped: 1 file(s)]");
}

#[tokio::test]
async fn files_above_the_size_limit_are_skipped_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    write_file(dir.path(), "a.txt", "hit\n");
    let huge = dir.path().join("huge.log");
    let file = std::fs::File::create(&huge).unwrap();
    file.set_len(MAX_FILESIZE + 1).unwrap();
    drop(file);

    let result = tool()
        .execute(&json!({ "pattern": "hit", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["largeSkipped"], json!(1));
    assert_eq!(data["totalMatches"], json!(1));
    assert_eq!(result.assistant_text(), "a.txt:1:hit\n[skipped: 1 file(s)]");
}

#[tokio::test]
async fn the_probe_stops_at_its_file_limit() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let body = gbk("中文内容\n");
    for index in 0..201 {
        write_bytes(dir.path(), &format!("zh/{:04}.txt", index), &body);
    }

    let result = tool()
        .execute(&json!({ "pattern": "no_such_token", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["nonUtf8Probe"]["files"], json!(200));
    assert_eq!(data["nonUtf8Probe"]["truncated"], json!(true));
    assert_eq!(
        result.assistant_text(),
        "(no matches)\n[non-utf8: 200 file(s)]",
        "the footer stays in its documented shape; truncation is only in `data`"
    );
}
