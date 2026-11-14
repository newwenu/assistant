use std::path::Path;

use assistant_tools::os::agent_glob::{AgentGlobTool, MAX_MATCHES};
use assistant_tools::{Tool, ToolAudience, ToolErrorKind, ToolTier, ToolUseContext};
use serde_json::json;

fn context(root: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: None,
        tool_call_id: Some("call-1".to_string()),
        ..Default::default()
    }
}

fn tool() -> AgentGlobTool {
    AgentGlobTool::new()
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dir");
    }
    std::fs::write(path, "x").expect("write fixture");
}

#[test]
fn schema_declares_pattern_path_and_exclude() {
    let schema = tool().input_schema();
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["pattern"]));
    assert!(schema["properties"]["pattern"].is_object());
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["exclude"].is_object());
    assert!(
        schema["properties"].get("action").is_none(),
        "schema must not declare the toolbox-injected action field"
    );
}

#[test]
fn tool_metadata_matches_the_readonly_contract() {
    let tool = tool();
    assert_eq!(tool.id(), "glob");
    assert_eq!(tool.tier(), ToolTier::Os);
    assert_eq!(tool.audience(), ToolAudience::BOTH);
    assert!(tool.behavior_hints().read_only);
    assert!(!tool.behavior_hints().destructive);
    assert!(!tool.behavior_hints().open_world);
    assert!(tool.is_concurrency_safe(None));
    assert_eq!(
        tool.permission_intents(&json!({ "pattern": "*" }), &ToolUseContext::default()),
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
        json!({ "pattern": "*", "path": 42 }),
        json!({ "pattern": "*", "exclude": "*.md" }),
        json!({ "pattern": "*", "exclude": [1] }),
        json!({ "pattern": "*.{md", "path": "." }),
    ];
    for input in cases {
        assert!(
            !tool.validate_input(&input, &ctx).ok,
            "expected rejection for {}",
            input
        );
    }

    assert!(
        tool.validate_input(&json!({ "pattern": "**/*.rs", "path": "." }), &ctx)
            .ok
    );
}

#[test]
fn validate_input_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());

    let result = tool().validate_input(
        &json!({ "pattern": "*", "path": other.path().display().to_string() }),
        &ctx,
    );
    assert!(!result.ok);
    assert!(result
        .message
        .unwrap_or_default()
        .contains("outside workspace"));
}

#[tokio::test]
async fn matches_are_sorted_by_path() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("src").join("z.ts"));
    touch(&dir.path().join("src").join("a.ts"));
    touch(&dir.path().join("src").join("nested").join("b.ts"));
    touch(&dir.path().join("docs").join("c.md"));

    let result = tool()
        .execute(&json!({ "pattern": "**/*.ts", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        result.assistant_text(),
        "src/a.ts\nsrc/nested/b.ts\nsrc/z.ts"
    );
    let data = result.data.unwrap();
    assert_eq!(data["totalMatches"], json!(3));
    assert_eq!(data["truncated"], json!(false));
}

#[tokio::test]
async fn a_pattern_without_a_slash_matches_at_any_depth() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("a.ts"));
    touch(&dir.path().join("src").join("deep").join("b.ts"));
    touch(&dir.path().join("src").join("deep").join("c.md"));

    let result = tool()
        .execute(&json!({ "pattern": "*.ts", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        result.assistant_text(),
        "a.ts\nsrc/deep/b.ts",
        "'*.ts' is normalized to '**/*.ts' so it recurses"
    );
}

#[tokio::test]
async fn a_pattern_with_a_slash_only_matches_that_depth() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("src").join("a.ts"));
    touch(&dir.path().join("src").join("nested").join("b.ts"));

    let result = tool()
        .execute(&json!({ "pattern": "src/*.ts", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(
        result.assistant_text(),
        "src/a.ts",
        "a pattern containing '/' must not recurse"
    );
}

#[tokio::test]
async fn exclude_filters_after_matching() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("src").join("a.ts"));
    touch(&dir.path().join("src").join("nested").join("b.ts"));

    let result = tool()
        .execute(
            &json!({ "pattern": "**/*.ts", "path": ".", "exclude": ["**/nested/**"] }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "src/a.ts");
}

#[tokio::test]
async fn gitignore_is_respected() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    std::fs::write(dir.path().join(".gitignore"), "ignored.txt\n").unwrap();
    touch(&dir.path().join("ignored.txt"));
    touch(&dir.path().join("kept.txt"));

    let result = tool()
        .execute(&json!({ "pattern": "**/*.txt", "path": "." }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "kept.txt");
}

#[tokio::test]
async fn defaults_the_base_directory_to_the_workspace_root() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("root.rs"));

    let result = tool()
        .execute(&json!({ "pattern": "**/*.rs" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "root.rs");
}

#[tokio::test]
async fn a_missing_workspace_root_is_a_business_error() {
    let ctx = ToolUseContext::default();
    let result = tool()
        .execute(&json!({ "pattern": "**/*.rs" }), &ctx)
        .await
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(result.error.unwrap_or_default().contains("path"));
}

#[tokio::test]
async fn results_are_capped_at_one_thousand_with_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    let bulk = dir.path().join("bulk");
    std::fs::create_dir(&bulk).unwrap();
    for index in 0..=MAX_MATCHES {
        touch(&bulk.join(format!("f{:04}.txt", index)));
    }

    let result = tool()
        .execute(&json!({ "pattern": "**/*.txt", "path": "bulk" }), &ctx)
        .await
        .unwrap();

    assert!(result.ok);
    let data = result.data.as_ref().unwrap();
    assert_eq!(data["totalMatches"], json!(MAX_MATCHES + 1));
    assert_eq!(data["truncated"], json!(true));
    assert_eq!(data["matches"].as_array().unwrap().len(), MAX_MATCHES);
    let text = result.assistant_text();
    assert!(text.contains(&format!(
        "[truncated: showing {} of {} items",
        MAX_MATCHES,
        MAX_MATCHES + 1
    )));
}

#[tokio::test]
async fn an_empty_result_is_reported_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path());
    touch(&dir.path().join("a.md"));

    let result = tool()
        .execute(&json!({ "pattern": "**/*.rs", "path": "." }), &ctx)
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
    touch(&dir.path().join("a.rs"));

    let result = tool()
        .execute(
            &json!({ "action": "glob", "pattern": "**/*.rs", "path": "." }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.assistant_text(), "a.rs");
}
