use assistant_tools::os::output::{truncate_output, OutputLimits};

fn limits(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> OutputLimits {
    OutputLimits::head(max_bytes, max_lines, preview_bytes)
}

#[test]
fn output_within_limits_is_returned_unchanged() {
    let text = "alpha\nbeta\ngamma\n";
    let output = truncate_output(text, &limits(1_000, 100, 0));

    assert_eq!(output.text, text);
    assert!(!output.truncated);
    assert_eq!(output.total_lines, 3);
    assert_eq!(output.total_bytes, text.len());
    assert_eq!(output.returned_lines, 3);
}

#[test]
fn line_limit_truncates_and_reports_actual_totals() {
    let text = "one\ntwo\nthree\nfour\n";
    let output = truncate_output(text, &limits(0, 2, 0));

    assert!(output.truncated);
    assert_eq!(output.total_lines, 4);
    assert_eq!(output.returned_lines, 2);
    assert!(output.text.starts_with("one\ntwo\n"));
    assert!(output.text.contains("showing 2 of 4 lines"));
    let expected_total = format!("of {} bytes", text.len());
    assert!(output.text.contains(&expected_total));
}

#[test]
fn byte_limit_truncates_and_reports_actual_totals() {
    let text = "one\ntwo\nthree\n";
    let output = truncate_output(text, &limits(8, 0, 0));

    assert!(output.truncated);
    assert_eq!(output.total_bytes, text.len());
    assert_eq!(output.returned_lines, 2);
    assert!(output.text.starts_with("one\ntwo\n"));
    assert!(output.text.contains("showing 2 of 3 lines"));
    let expected_total = format!("8 of {} bytes", text.len());
    assert!(output.text.contains(&expected_total));
}

/// A8：产出侧三处口径逐字节一致（I73 §3.6 R1–R3 / §7）。
///
/// 标签里的路径、`compact.persisted.path`、`PersistedRef::model_ref()` 必须同一个值，
/// 且该值不含本机绝对路径——否则模型回读与卡片展示会指向两个不同的东西。
#[tokio::test]
async fn a8_tag_compact_and_model_ref_share_one_value() {
    use assistant_tools::compact::{build_compact, parse_persisted_tag};
    use assistant_tools::storage::{
        persisted_reference_from_tail, FileToolResultStorage, PersistToolResult, ToolResultStorage,
    };
    use assistant_tools::ToolResult;
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let content = "line\n".repeat(2_000);
    let reference = storage
        .persist(PersistToolResult {
            tool_call_id: "call-a8".to_string(),
            tool_name: "shell".to_string(),
            session_id: Some("session-a8".to_string()),
            content: content.clone(),
        })
        .await
        .unwrap();

    let text =
        persisted_reference_from_tail(&reference.model_ref(), &content, content.chars().count());
    let result =
        ToolResult::ok_with_assistant(json!({ "outputPath": reference.path }), text.clone())
            .with_persisted(Some(reference.clone()));
    let compact = build_compact("shell", &json!({}), &result, result.persisted.as_ref());

    let tag = parse_persisted_tag(&text).expect("tag must parse").path;
    assert_eq!(tag, reference.model_ref(), "标签路径必须就是 model_ref");
    assert_eq!(
        compact.persisted.as_ref().map(|info| info.path.as_str()),
        Some(reference.model_ref().as_str()),
        "compact 必须与标签同源"
    );
    assert!(
        !tag.contains(dir.path().to_str().unwrap()),
        "模型可见的路径不得带本机绝对路径: {tag}"
    );
    assert!(
        reference.path.starts_with(dir.path().to_str().unwrap()),
        "PersistedRef::path 仍必须是绝对路径（R4）"
    );
}
