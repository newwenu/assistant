use std::path::Path;
use std::time::Duration;

use assistant_tools::os::agent_grep::{AgentGrepTool, MAX_MATCHES};
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

#[test]
fn schema_declares_pattern_path_include_and_context() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["pattern"]));
    assert!(schema["properties"]["pattern"].is_object());
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["include"].is_object());
    assert!(schema["properties"]["context"].is_object());
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
        json!({ "pattern": "hit(", "path": "." }),
        json!({ "pattern": "hit", "include": "*.{rs", "path": "." }),
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
