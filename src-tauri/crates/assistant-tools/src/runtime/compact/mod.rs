use serde_json::Value;

use crate::result::{ToolErrorKind, ToolResult};
use crate::storage::PersistedRef;

mod anchor;
mod head;
mod preview;
mod truncation;
mod types;

use anchor::build_anchor;

pub use head::build_head;
pub use preview::args_preview;
pub use truncation::{parse_persisted_tag, parse_truncation};
pub use types::{
    DocAnchor, LineRange, PersistedInfo, ToolResultCompact, TruncatedInfo, TruncatedUnit,
    ARGS_PREVIEW_MAX_CHARS, HEAD_MAX_CHARS, SUBJECT_MAX_CHARS,
};

/// 由结果事实渲染信封。`persisted` 必须来自存储后端返回的引用，
/// 不得由文本解析产生（门禁 8）。
pub fn build_compact(
    tool: &str,
    arguments: &Value,
    result: &ToolResult,
    persisted: Option<&PersistedRef>,
) -> ToolResultCompact {
    let text = result.assistant_text();
    let data = result.data.as_ref();
    ToolResultCompact {
        head: build_head(tool, arguments, data, result),
        truncated: parse_truncation(&text),
        persisted: persisted.map(|reference| PersistedInfo {
            path: reference.path.clone(),
            bytes: Some(reference.bytes as u64),
        }),
        error_kind: if result.ok {
            None
        } else {
            Some(result.error_kind.unwrap_or(ToolErrorKind::Business))
        },
        anchor: build_anchor(tool, arguments, data),
    }
}

/// 从历史行重建信封：结果文本 + 参数 + 状态。`error_kind` 只在文本可判定时给出。
///
/// 这条路径是读取回退（§3.4 第三行），允许解析 `<persisted-output>` 标签；
/// 标签本身由 `PersistedRef` 渲染，round-trip 保证一致。
pub fn rebuild_compact(
    tool: &str,
    arguments: &Value,
    result_text: Option<&str>,
    is_error: bool,
) -> ToolResultCompact {
    let text = result_text.unwrap_or_default();
    let result = if is_error {
        ToolResult {
            ok: false,
            data: None,
            result_for_assistant: None,
            error: Some(text.to_string()),
            error_kind: error_kind_from_text(text),
        }
    } else {
        ToolResult {
            ok: true,
            data: None,
            result_for_assistant: Some(text.to_string()),
            error: None,
            error_kind: None,
        }
    };
    ToolResultCompact {
        head: build_head(tool, arguments, None, &result),
        truncated: parse_truncation(text),
        persisted: parse_persisted_tag(text),
        error_kind: result.error_kind,
        anchor: None,
    }
}

/// 历史错误的分类：写入侧 `ToolResult::from_exec_error_kind` 的文案前缀是稳定契约，
/// 读取侧据此还原同一分类（门禁 9）。未知文案归 `Business`。
pub fn error_kind_from_text(text: &str) -> Option<ToolErrorKind> {
    let text = text.trim_start();
    if !text.starts_with("tool '") {
        return Some(ToolErrorKind::Business);
    }
    let message = text.split_once("': ").map(|(_, rest)| rest).unwrap_or(text);
    if message.starts_with("permission denied for") || message.starts_with("rejected by user") {
        return Some(ToolErrorKind::Denied);
    }
    if message.starts_with("approval timed out") {
        return Some(ToolErrorKind::Timeout);
    }
    Some(ToolErrorKind::Internal)
}

fn data_str(data: Option<&Value>, key: &str) -> Option<String> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn data_u64(data: Option<&Value>, key: &str) -> Option<u64> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

fn data_bool(data: Option<&Value>, key: &str) -> Option<bool> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_bool)
}

fn args_str(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn truncate_end(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

/// 中间省略：首 40 + `…` + 尾 38（`SUBJECT_MAX_CHARS` 的既定口径）。
fn truncate_middle(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let head_chars = max_chars / 2;
    let tail_chars = max_chars.saturating_sub(head_chars + 1);
    let head: String = text.chars().take(head_chars).collect();
    let tail: String = text
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}…{}", head, tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn persisted(path: &str, bytes: usize) -> PersistedRef {
        PersistedRef {
            path: path.to_string(),
            bytes,
        }
    }

    #[test]
    fn ok_head_uses_subject_and_scale_for_read() {
        let result = ToolResult::ok_with_assistant(
            json!({
                "path": "src/a.rs",
                "startLine": 1,
                "endLine": 120,
                "totalLines": 120
            }),
            "body",
        );
        let compact = build_compact("read", &json!({ "path": "src/a.rs" }), &result, None);

        assert_eq!(
            compact.head,
            "ok read · src/a.rs:1-120 · 120/120 lines · 4 B"
        );
        assert!(!compact.head.contains('\n'));
        assert!(compact.head.chars().count() <= HEAD_MAX_CHARS);
        assert!(compact.error_kind.is_none());
        assert_eq!(compact.anchor.unwrap().ranges[0].start, 1);
    }

    #[test]
    fn failed_head_carries_kind_and_reason() {
        let result = ToolResult::from_exec_error_kind(
            "edit",
            ToolErrorKind::Denied,
            "permission denied for 'edit' on 'src/b.ts'",
        );
        let compact = build_compact("edit", &json!({ "path": "src/b.ts" }), &result, None);

        assert!(compact.head.starts_with("err denied edit · src/b.ts · "));
        assert_eq!(compact.error_kind, Some(ToolErrorKind::Denied));
        assert!(!compact.head.contains('\n'));
    }

    #[test]
    fn head_is_single_line_even_when_error_has_multiple_lines() {
        let result = ToolResult::err("first line\nsecond line\nthird");
        let compact = build_compact("glob", &json!({ "pattern": "**/*.rs" }), &result, None);

        assert!(!compact.head.contains('\n'));
        assert!(compact.head.contains("first line"));
        assert!(!compact.head.contains("second line"));
    }

    #[test]
    fn head_is_capped_at_the_budget() {
        let result = ToolResult::err("x".repeat(400));
        let compact = build_compact(
            "grep",
            &json!({ "pattern": "p".repeat(200) }),
            &result,
            None,
        );

        assert_eq!(compact.head.chars().count(), HEAD_MAX_CHARS);
    }

    #[test]
    fn truncation_notice_maps_to_lines_unit() {
        let info = parse_truncation("[truncated: showing 2 of 4 lines, 8 of 100 bytes]").unwrap();

        assert_eq!(info.unit, TruncatedUnit::Lines);
        assert_eq!(info.returned, Some(2));
        assert_eq!(info.total, Some(4));
    }

    #[test]
    fn explicit_unknown_notice_drops_half_known_numbers() {
        let info =
            parse_truncation("[truncated: showing 120 lines, 3.2 KB; total unknown]").unwrap();

        assert_eq!(info.unit, TruncatedUnit::Unknown);
        assert_eq!(info.returned, None);
        assert_eq!(info.total, None);
    }

    #[test]
    fn legacy_match_wording_is_projected_as_items() {
        let info = parse_truncation("[truncated: showing 10 of 25 matches; narrow it]").unwrap();

        assert_eq!(info.unit, TruncatedUnit::Items);
        assert_eq!(info.returned, Some(10));
        assert_eq!(info.total, Some(25));
    }

    #[test]
    fn unknown_unit_never_carries_numbers() {
        let info = parse_truncation("[truncated: showing 5 of 10 widgets]").unwrap();

        assert_ne!(info.unit, TruncatedUnit::Unknown);
        assert_eq!(info.returned, Some(5));
        assert_eq!(info.total, Some(10));
    }

    #[test]
    fn absent_truncation_notice_means_null() {
        assert!(parse_truncation("body without notice").is_none());
        let result = ToolResult::ok_with_assistant(json!({ "totalLines": 3 }), "body");
        let compact = build_compact("read", &json!({ "path": "a.rs" }), &result, None);
        assert!(compact.truncated.is_none());
    }

    /// 门禁 7（口径分离）：`skipped` / `incomplete` 是工具自述口径，只能出现在
    /// body 或 head 的 `scale`，不得混进 `[truncated:` 提示的数字里。
    #[test]
    fn skipped_and_incomplete_wording_is_not_a_truncation_notice() {
        assert!(parse_truncation("a.txt:1:hit\n[skipped: 2 file(s)]").is_none());

        let incomplete =
            "hits\n[search stopped at the 500ms time budget; results may be incomplete]";
        assert!(parse_truncation(incomplete).is_none());

        let both = "a.txt:1:hit\n[skipped: 2 file(s)]\n\
                    [truncated: showing 10 of 25 lines, 400 of 900 bytes]";
        let info = parse_truncation(both).unwrap();
        assert_eq!(info.unit, TruncatedUnit::Lines);
        assert_eq!(info.returned, Some(10));
        assert_eq!(info.total, Some(25));
    }

    /// 门禁 4（round-trip）：渲染出的三态提示必须能被 `parse_truncation` 原样读回。
    #[test]
    fn rendered_truncation_notice_round_trips_across_strategies() {
        use crate::os::output::{truncate_output, OutputLimits, TruncationStrategy};

        let text = "one\ntwo\nthree\nfour\n";
        for strategy in [
            TruncationStrategy::Head,
            TruncationStrategy::Tail,
            TruncationStrategy::HeadTail,
        ] {
            let limits = OutputLimits {
                max_bytes: 0,
                max_lines: 2,
                preview_bytes: 0,
                strategy,
            };
            let rendered = truncate_output(text, &limits);
            assert!(rendered.truncated, "strategy {strategy:?} should truncate");

            let info = parse_truncation(&rendered.text).unwrap_or_else(|| {
                panic!("strategy {strategy:?} notice must parse: {}", rendered.text)
            });
            assert_eq!(info.unit, TruncatedUnit::Lines, "strategy {strategy:?}");
            assert_eq!(
                info.returned,
                Some(rendered.returned_lines as u64),
                "strategy {strategy:?}"
            );
            assert_eq!(
                info.total,
                Some(rendered.total_lines as u64),
                "strategy {strategy:?}"
            );
        }

        let limits = OutputLimits {
            max_bytes: 1_000,
            max_lines: 100,
            preview_bytes: 0,
            strategy: TruncationStrategy::Head,
        };
        let rendered = truncate_output(text, &limits);
        assert!(
            parse_truncation(&rendered.text).is_none(),
            "no omission, no notice"
        );
    }

    #[test]
    fn persisted_comes_from_the_reference_not_from_text() {
        let reference = persisted("results/abc.txt", 12_345);
        let result = ToolResult::ok_with_assistant(
            json!({ "totalLines": 1 }),
            "<persisted-output path=\"results/abc.txt\" bytes=\"12345\">\npreview",
        );
        let compact = build_compact(
            "read",
            &json!({ "path": "a.rs" }),
            &result,
            Some(&reference),
        );

        let info = compact.persisted.unwrap();
        assert_eq!(info.path, "results/abc.txt");
        assert_eq!(info.bytes, Some(12_345));
    }

    /// 门禁 4（round-trip）：三态 × 落盘/不落盘 × 成败的每个组合，
    /// 信封里的 `truncated` / `persisted` / `errorKind` 都必须与「从正文重新解析」一致。
    /// 这条不变式是历史回退路径（§3.4 第三行）成立的前提。
    #[test]
    fn envelope_matches_text_reparse_for_every_combination() {
        let bodies = [
            ("body without notice", None),
            (
                "body\n[truncated: showing 2 of 4 lines, 8 of 100 bytes]",
                Some((TruncatedUnit::Lines, Some(2), Some(4))),
            ),
            (
                "body\n[truncated: showing 120 lines, 3.2 KB; total unknown]",
                Some((TruncatedUnit::Unknown, None, None)),
            ),
        ];

        for (body, expected) in bodies {
            for reference in [None, Some(persisted("results/abc.txt", 12_345))] {
                for failed in [false, true] {
                    let result = if failed {
                        ToolResult::from_exec_error_kind(
                            "read",
                            ToolErrorKind::Denied,
                            format!("permission denied for a.rs\n{body}"),
                        )
                    } else {
                        ToolResult::ok_with_assistant(
                            json!({ "path": "a.rs", "totalLines": 4 }),
                            body,
                        )
                    };
                    let arguments = json!({ "path": "a.rs" });
                    let compact = build_compact("read", &arguments, &result, reference.as_ref());

                    match expected {
                        None => assert!(
                            compact.truncated.is_none(),
                            "无提示时不得产出 truncated：{body}"
                        ),
                        Some((unit, returned, total)) => {
                            let info = compact.truncated.expect("提示存在时必须有 truncated");
                            assert_eq!(info.unit, unit, "unit 必须与文本一致：{body}");
                            assert_eq!(info.returned, returned, "returned：{body}");
                            assert_eq!(info.total, total, "total：{body}");
                        }
                    }

                    assert_eq!(
                        compact.persisted.is_some(),
                        reference.is_some(),
                        "persisted 只由引用决定，与文本无关"
                    );
                    assert_eq!(
                        compact.error_kind.is_some(),
                        failed,
                        "errorKind 只由成败决定"
                    );
                }
            }
        }
    }

    #[test]
    fn persisted_tag_round_trips() {
        let tag = "<persisted-output path=\"results/abc.txt\" bytes=\"12345\">\npreview";
        let info = parse_persisted_tag(tag).unwrap();

        assert_eq!(info.path, "results/abc.txt");
        assert_eq!(info.bytes, Some(12_345));
        let rendered = format!(
            "<persisted-output path=\"{}\" bytes=\"{}\">",
            info.path,
            info.bytes.unwrap()
        );
        assert_eq!(
            rendered,
            "<persisted-output path=\"results/abc.txt\" bytes=\"12345\">"
        );
    }

    /// 门禁 11（锚点结果时刻）：`edit` 的行区间只取结果里的实际写入行，
    /// 即使调用参数里带了行号也一律不采信——调用期间文件被外部改动时，
    /// 参数里的行号已过期，按参数估算会指到错误位置。
    #[test]
    fn edit_anchor_uses_result_time_lines() {
        let result = ToolResult::ok_with_assistant(
            json!({
                "path": "src/b.ts",
                "startLine": 40,
                "oldEndLine": 42,
                "newEndLine": 45,
                "matchCount": 1
            }),
            "diff",
        );
        let arguments = json!({
            "path": "src/b.ts",
            "startLine": 1,
            "endLine": 3,
            "offset": 1,
            "limit": 3
        });
        let compact = build_compact("edit", &arguments, &result, None);

        let anchor = compact.anchor.unwrap();
        assert_eq!(anchor.path, "src/b.ts");
        assert_eq!(anchor.ranges, vec![LineRange { start: 40, end: 45 }]);
    }

    /// 门禁 11：结果没给行区间时宁可不产出锚点，也不回落到 `args` 的行号。
    #[test]
    fn anchor_is_absent_when_result_has_no_line_range() {
        let result = ToolResult::ok_with_assistant(json!({ "path": "src/b.ts" }), "diff");
        let arguments = json!({ "path": "src/b.ts", "startLine": 1, "endLine": 9 });

        assert!(build_compact("edit", &arguments, &result, None)
            .anchor
            .is_none());
        assert!(build_compact("read", &arguments, &result, None)
            .anchor
            .is_none());
    }

    #[test]
    fn multi_file_tools_have_no_anchor() {
        let result = ToolResult::ok_with_assistant(
            json!({ "path": ".", "pattern": "needle", "totalMatches": 3 }),
            "hits",
        );
        let compact = build_compact("grep", &json!({ "pattern": "needle" }), &result, None);

        assert!(compact.anchor.is_none());
        assert_eq!(compact.head, "ok grep · needle @ . · 3 hits");
    }

    #[test]
    fn args_preview_is_bounded_and_skips_absent_keys() {
        let preview = args_preview(
            "edit",
            &json!({ "path": "src/b.ts", "old_string": "x".repeat(500) }),
        )
        .unwrap();

        assert!(preview.starts_with("path=src/b.ts"));
        assert!(!preview.contains("old_string"));
        assert!(preview.chars().count() <= ARGS_PREVIEW_MAX_CHARS);
    }

    #[test]
    fn args_preview_is_null_when_no_key_matches() {
        assert!(args_preview("read", &json!({})).is_none());
    }

    #[test]
    fn error_kind_is_replayable_from_text() {
        assert_eq!(
            error_kind_from_text("tool 'edit': permission denied for 'edit' on 'a.ts'"),
            Some(ToolErrorKind::Denied)
        );
        assert_eq!(
            error_kind_from_text("tool 'edit': rejected by user: no"),
            Some(ToolErrorKind::Denied)
        );
        assert_eq!(
            error_kind_from_text("tool 'edit': approval timed out after 30s"),
            Some(ToolErrorKind::Timeout)
        );
        assert_eq!(
            error_kind_from_text("tool 'read': unknown tool 'read'"),
            Some(ToolErrorKind::Internal)
        );
        assert_eq!(
            error_kind_from_text("'old_string' was not found"),
            Some(ToolErrorKind::Business)
        );
    }

    #[test]
    fn rebuild_from_history_keeps_head_shape_and_kind() {
        let rebuilt = rebuild_compact("read", &json!({ "path": "src/a.rs" }), Some("body"), false);

        assert!(rebuilt.head.starts_with("ok read · src/a.rs"));
        assert!(!rebuilt.head.contains('\n'));
        assert!(rebuilt.head.chars().count() <= HEAD_MAX_CHARS);

        let failed = rebuild_compact(
            "edit",
            &json!({ "path": "src/b.ts" }),
            Some("tool 'edit': permission denied for 'edit' on 'src/b.ts'"),
            true,
        );
        assert_eq!(failed.error_kind, Some(ToolErrorKind::Denied));
    }
}
