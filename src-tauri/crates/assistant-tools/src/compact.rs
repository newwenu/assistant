use serde_json::Value;

use crate::os::output::human_bytes;
use crate::result::{ToolErrorKind, ToolResult};
use crate::storage::PersistedRef;

/// head 单行上限（字符）：单行 + 160 字符是信封的硬约束（I46 compact §3.1）。
pub const HEAD_MAX_CHARS: usize = 160;
/// subject 上限（字符）：超长按中间省略。
pub const SUBJECT_MAX_CHARS: usize = 80;
/// `argsPreview` 上限（字符）：Started 首帧只带参数摘要。
pub const ARGS_PREVIEW_MAX_CHARS: usize = 200;
/// 单个参数值上限（字符）：避免一个长文本把整条预览挤满。
const PREVIEW_VALUE_MAX_CHARS: usize = 60;
const HEAD_SEPARATOR: &str = " · ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedUnit {
    Lines,
    Items,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncatedInfo {
    pub unit: TruncatedUnit,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedInfo {
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocAnchor {
    pub path: String,
    pub ranges: Vec<LineRange>,
}

/// 一次调用的紧凑信封：展示与历史重载的唯一事实源（I46 compact §3.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResultCompact {
    pub head: String,
    pub truncated: Option<TruncatedInfo>,
    pub persisted: Option<PersistedInfo>,
    pub error_kind: Option<ToolErrorKind>,
    pub anchor: Option<DocAnchor>,
}

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

/// head：`ok <tool> · <subject> · <scale>` / `err <kind> <tool> · <subject> · <reason>`。
pub fn build_head(
    tool: &str,
    arguments: &Value,
    data: Option<&Value>,
    result: &ToolResult,
) -> String {
    if result.ok {
        ok_head(tool, arguments, data, result)
    } else {
        err_head(tool, arguments, data, result)
    }
}

fn ok_head(tool: &str, arguments: &Value, data: Option<&Value>, result: &ToolResult) -> String {
    let mut parts = vec![format!("ok {}", tool)];
    if let Some(subject) = subject(tool, arguments, data) {
        parts.push(truncate_middle(&subject, SUBJECT_MAX_CHARS));
    }
    if let Some(scale) = scale(tool, data, result) {
        parts.push(scale);
    }
    truncate_end(&parts.join(HEAD_SEPARATOR), HEAD_MAX_CHARS)
}

fn err_head(tool: &str, arguments: &Value, data: Option<&Value>, result: &ToolResult) -> String {
    let kind = result.error_kind.unwrap_or(ToolErrorKind::Business);
    let reason = result
        .error
        .as_deref()
        .unwrap_or("failed")
        .lines()
        .next()
        .unwrap_or("failed")
        .trim()
        .to_string();
    let mut parts = vec![format!("err {} {}", kind_word(kind), tool)];
    if let Some(subject) = subject(tool, arguments, data) {
        parts.push(truncate_middle(&subject, SUBJECT_MAX_CHARS));
    }
    parts.push(reason);
    truncate_end(&parts.join(HEAD_SEPARATOR), HEAD_MAX_CHARS)
}

fn kind_word(kind: ToolErrorKind) -> &'static str {
    match kind {
        ToolErrorKind::Business => "business",
        ToolErrorKind::Timeout => "timeout",
        ToolErrorKind::Cancelled => "cancelled",
        ToolErrorKind::Internal => "internal",
        ToolErrorKind::Denied => "denied",
    }
}

/// subject 优先取结果事实（`data`），失败结果没有 `data` 时回退到参数。
fn subject(tool: &str, arguments: &Value, data: Option<&Value>) -> Option<String> {
    match tool {
        "read" => {
            let path = data_str(data, "path").or_else(|| args_str(arguments, "path"))?;
            let start = data_u64(data, "startLine");
            let end = data_u64(data, "endLine");
            Some(match (start, end) {
                (Some(start), Some(end)) => format!("{}:{}-{}", path, start, end),
                _ => path,
            })
        }
        "write" | "edit" | "mkdir" | "delete" | "list" => {
            data_str(data, "path").or_else(|| args_str(arguments, "path"))
        }
        "grep" => {
            let pattern = data_str(data, "pattern").or_else(|| args_str(arguments, "pattern"))?;
            let path = data_str(data, "path")
                .or_else(|| args_str(arguments, "path"))
                .unwrap_or_else(|| ".".to_string());
            Some(format!("{} @ {}", pattern, path))
        }
        "glob" => data_str(data, "pattern").or_else(|| args_str(arguments, "pattern")),
        "move" => {
            let source = data_str(data, "source").or_else(|| args_str(arguments, "source"))?;
            let target = data_str(data, "target").or_else(|| args_str(arguments, "target"))?;
            Some(format!("{} -> {}", source, target))
        }
        _ => args_str(arguments, "path")
            .or_else(|| args_str(arguments, "pattern"))
            .or_else(|| args_str(arguments, "query")),
    }
}

fn scale(tool: &str, data: Option<&Value>, result: &ToolResult) -> Option<String> {
    match tool {
        "read" => {
            let total = data_u64(data, "totalLines")?;
            let start = data_u64(data, "startLine").unwrap_or(1);
            let end = data_u64(data, "endLine").unwrap_or(total);
            let returned = end.saturating_sub(start).saturating_add(1);
            Some(format!(
                "{}/{} lines · {}",
                returned,
                total,
                human_bytes(result.assistant_text().len())
            ))
        }
        "write" => {
            let lines = data_u64(data, "lines")?;
            let state = if data_bool(data, "created") == Some(true) {
                "created"
            } else if data_bool(data, "appended") == Some(true) {
                "appended"
            } else if data_bool(data, "unchanged") == Some(true) {
                "unchanged"
            } else {
                "updated"
            };
            Some(format!("{} · {} lines", state, lines))
        }
        "edit" => {
            let count = data_u64(data, "matchCount")?;
            Some(if count == 1 {
                "1 replacement".to_string()
            } else {
                format!("{} replacements", count)
            })
        }
        "grep" => {
            let hits = data_u64(data, "totalMatches")?;
            Some(match data_u64(data, "files") {
                Some(files) => format!("{} hits · {} files", hits, files),
                None => format!("{} hits", hits),
            })
        }
        "glob" => Some(format!("{} paths", data_u64(data, "totalMatches")?)),
        "list" => {
            let entries = data_u64(data, "totalEntries")?;
            Some(match data_u64(data, "depth") {
                Some(depth) => format!("{} entries · depth {}", entries, depth),
                None => format!("{} entries", entries),
            })
        }
        "mkdir" => Some(if data_bool(data, "created") == Some(true) {
            "created".to_string()
        } else {
            "already exists".to_string()
        }),
        "move" => Some(if data_bool(data, "overwritten") == Some(true) {
            "moved (overwritten)".to_string()
        } else {
            "moved".to_string()
        }),
        "delete" => {
            let entries = data_u64(data, "entriesDeleted")?;
            let bytes = data_u64(data, "bytesDeleted");
            Some(match bytes {
                Some(bytes) => format!("{} entries · {}", entries, human_bytes(bytes as usize)),
                None => format!("{} entries", entries),
            })
        }
        "ask_user" => {
            let count = data
                .and_then(|value| value.get("questions"))
                .and_then(Value::as_array)
                .map(|questions| questions.len() as u64)?;
            Some(format!("{} questions", count))
        }
        _ => None,
    }
}

/// 锚点只在结果时刻取（门禁 11）；`edit` 的行区间来自实际写入，不由 `args` 估算。
fn build_anchor(tool: &str, arguments: &Value, data: Option<&Value>) -> Option<DocAnchor> {
    let path = data_str(data, "path").or_else(|| args_str(arguments, "path"))?;
    match tool {
        "edit" => {
            let start = data_u64(data, "startLine")?;
            let end = data_u64(data, "newEndLine")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange { start, end }],
            })
        }
        "write" => {
            let lines = data_u64(data, "lines")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange {
                    start: 1,
                    end: lines.max(1),
                }],
            })
        }
        "read" => {
            let start = data_u64(data, "startLine")?;
            let end = data_u64(data, "endLine")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange { start, end }],
            })
        }
        _ => None,
    }
}

/// `[truncated: showing 2 of 4 lines, 8 of 100 bytes]` → 三态。
///
/// 三态口径（I46 compact §2.2 D2，门禁 3）：
/// - 无 `[truncated:` → `None`；
/// - 含 `total unknown` → `unit = Unknown`，`returned` / `total` 均为 `None`；
/// - 其余为精确态，`returned` / `total` 均有值。
pub fn parse_truncation(text: &str) -> Option<TruncatedInfo> {
    let start = text.rfind("[truncated:")?;
    let body = &text[start + "[truncated:".len()..];
    let body = body.split(']').next().unwrap_or_default().trim();

    // 显式未知优先：无法计数时不得保留半套数字（否则「未知」与「精确」无法区分）。
    if body.contains("total unknown") {
        return Some(TruncatedInfo {
            unit: TruncatedUnit::Unknown,
            returned: None,
            total: None,
        });
    }

    let mut returned = None;
    let mut total = None;
    let mut unit = TruncatedUnit::Unknown;
    if let Some(rest) = body.strip_prefix("showing ") {
        let mut halves = rest.splitn(2, " of ");
        if let (Some(returned_text), Some(total_text)) = (halves.next(), halves.next()) {
            returned = returned_text.trim().parse::<u64>().ok();
            let total_text = total_text.trim();
            let digits: String = total_text
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect();
            total = digits.parse::<u64>().ok();
            if total_text.contains("line") {
                unit = TruncatedUnit::Lines;
            } else if total_text.contains("item")
                || total_text.contains("match")
                || total_text.contains("file")
                || total_text.contains("entry")
            {
                // `match` / `file` / `entry` 是条目计数的同义写法，统一归入 `Items`。
                unit = TruncatedUnit::Items;
            } else if total_text.contains("byte") {
                unit = TruncatedUnit::Bytes;
            }
        }
    }
    if unit == TruncatedUnit::Unknown && returned.is_some() && total.is_some() {
        // 旧文本带数字但单位词不认识：归入条目计数。
        // `Unknown` 只保留给显式 `total unknown`，否则门禁 3 的双向等价不成立。
        unit = TruncatedUnit::Items;
    }
    Some(TruncatedInfo {
        unit,
        returned,
        total,
    })
}

/// `<persisted-output path="..." bytes="N">` → 持久化投影（仅文本历史回退用）。
pub fn parse_persisted_tag(text: &str) -> Option<PersistedInfo> {
    let start = text.find("<persisted-output")?;
    let body = &text[start..];
    let end = body.find('>')?;
    let tag = &body[..end];
    let path = attribute(tag, "path")?;
    let bytes = attribute(tag, "bytes").and_then(|value| value.parse::<u64>().ok());
    Some(PersistedInfo { path, bytes })
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// `ArgsDelta` 的受控回退：只抽关键参数，总长 ≤ 200 字符。
pub fn args_preview(tool: &str, arguments: &Value) -> Option<String> {
    let object = arguments.as_object()?;
    let mut parts: Vec<String> = Vec::new();
    for key in preview_keys(tool) {
        let Some(value) = object.get(*key) else {
            continue;
        };
        let Some(rendered) = render_value(value) else {
            continue;
        };
        parts.push(format!("{}={}", key, rendered));
    }
    if parts.is_empty() {
        return None;
    }
    Some(truncate_end(&parts.join(", "), ARGS_PREVIEW_MAX_CHARS))
}

fn preview_keys(tool: &str) -> &'static [&'static str] {
    match tool {
        "read" => &["path", "offset", "limit"],
        "write" => &["path", "append"],
        "edit" => &["path", "replace_all"],
        "grep" => &["pattern", "path", "include"],
        "glob" => &["pattern", "path", "exclude"],
        "list" => &["path", "depth"],
        "mkdir" => &["path", "parents"],
        "move" => &["source", "target"],
        "delete" => &["path", "recursive"],
        "ask_user" => &["questions"],
        _ => &["path", "pattern", "query", "command", "source", "target"],
    }
}

fn render_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(truncate_end(text, PREVIEW_VALUE_MAX_CHARS)),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => Some(format!("[{} items]", items.len())),
        Value::Object(map) => Some(format!("{{{} keys}}", map.len())),
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
