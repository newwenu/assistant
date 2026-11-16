use crate::context::ToolUseContext;
use crate::os::agent_read::LINE_TRUNCATED_MARKER;
use crate::os::path;
use crate::r#trait::ApprovalPreview;

use super::locate::strip_read_line_prefixes;
use super::text::{line_index_at, line_spans, pattern_lines};
use super::{EditRequest, CONTEXT_LINES, MAX_DIFF_LINES, MAX_MATCH_CONTEXTS};

pub(super) fn diff_text(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut rendered = String::new();
    for line in old_lines.iter().take(MAX_DIFF_LINES) {
        rendered.push_str(&format!("- {}\n", line));
    }
    for line in new_lines.iter().take(MAX_DIFF_LINES) {
        rendered.push_str(&format!("+ {}\n", line));
    }
    let omitted = old_lines.len().saturating_sub(MAX_DIFF_LINES)
        + new_lines.len().saturating_sub(MAX_DIFF_LINES);
    if omitted > 0 {
        rendered.push_str(&format!("... {} more lines omitted\n", omitted));
    }
    rendered.trim_end().to_string()
}

pub(super) fn build_preview(
    request: &EditRequest,
    context: &ToolUseContext,
) -> Option<ApprovalPreview> {
    let resolved = path::resolve(&request.path, context).ok()?;
    let old_lines = request.old_string.lines().count().max(1);
    let new_lines = request.new_string.lines().count();
    let diff = diff_text(&request.old_string, &request.new_string);
    Some(ApprovalPreview {
        summary: format!(
            "编辑 {}：{} 行 → {} 行（按参数估算）\n{}",
            resolved.display(),
            old_lines,
            new_lines,
            diff
        ),
        truncated: old_lines > MAX_DIFF_LINES || new_lines > MAX_DIFF_LINES,
        estimated: true,
    })
}

pub(super) fn not_found_message(path_text: &str, content: &str, old: &str) -> String {
    let mut message = format!(
        "'old_string' was not found in '{}' after trying exact, line-prefix-stripped, \
         trailing-newline, trim-end, trim and normalized matching. The file may have changed \
         since your last read, or the text drifted; call read again and copy the exact current \
         text.",
        path_text
    );

    if strip_read_line_prefixes(old).is_some() {
        message.push_str(
            "\nHint: 'old_string' looks like read output with line-number prefixes; copy the \
             text without the prefix column.",
        );
    }
    if old.contains(LINE_TRUNCATED_MARKER.trim()) {
        message.push_str(
            "\nHint: 'old_string' contains a truncated-read marker; read that range again to get \
             the complete line.",
        );
    }
    if let Some(marker) = trailing_footer_marker(old) {
        message.push_str(&format!(
            "\nHint: 'old_string' ends with the '{}' footer line that read appends after \
             non-UTF-8 output; that footer is not part of the file. Remove it and retry.",
            marker
        ));
    }
    if let Some(snippet) = nearest_snippet(content, old) {
        message.push_str("\nNearest text in the file:\n");
        message.push_str(&snippet);
    }
    message
}

/// footer 标记集（上级 §14.4 规则 5）：`[<tag>: …]` 独占一行。
///
/// 新增 tag 必须同步回写 §14.4；这里只识别、不剥离——自动剥离会把模型的误解静默修正为
/// 成功（规则 4），因此仅在 0 匹配诊断里指出。
const FOOTER_TAGS: &[&str] = &["encoding", "non-utf8"];

fn trailing_footer_marker(old: &str) -> Option<&str> {
    let last = old.trim_end_matches(['\n', '\r']).lines().last()?.trim();
    let inner = last.strip_prefix('[')?.strip_suffix(']')?;
    let tag = inner.split_once(':')?.0.trim();
    FOOTER_TAGS.contains(&tag).then_some(last)
}

pub(super) fn ambiguous_message(path_text: &str, content: &str, hits: &[(usize, usize)]) -> String {
    let mut message = format!(
        "'old_string' matches {} locations in '{}'; include more surrounding context to make it \
         unique, or pass replace_all = true.",
        hits.len(),
        path_text
    );
    for hit in hits.iter().take(MAX_MATCH_CONTEXTS) {
        message.push_str("\n\n");
        message.push_str(&render_context(content, line_index_at(content, hit.0)));
    }
    if hits.len() > MAX_MATCH_CONTEXTS {
        message.push_str(&format!(
            "\n\n... {} more matches omitted.",
            hits.len() - MAX_MATCH_CONTEXTS
        ));
    }
    message
}

fn nearest_snippet(content: &str, old: &str) -> Option<String> {
    let pattern = pattern_lines(old);
    let first = pattern.first().copied().unwrap_or("");
    let last = pattern.last().copied().unwrap_or("");
    let spans = line_spans(content);

    let mut best: Option<(usize, usize)> = None;
    for (index, (start, end, _)) in spans.iter().enumerate() {
        let line = &content[*start..*end];
        let score = shared_prefix_len(line, first) + shared_suffix_len(line, last);
        if score == 0 {
            continue;
        }
        let take = match best {
            Some((best_score, _)) => score > best_score,
            None => true,
        };
        if take {
            best = Some((score, index));
        }
    }
    best.map(|(_, index)| render_context(content, index))
}

fn shared_prefix_len(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

fn shared_suffix_len(left: &str, right: &str) -> usize {
    left.chars()
        .rev()
        .zip(right.chars().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn render_context(content: &str, center: usize) -> String {
    let spans = line_spans(content);
    if spans.is_empty() {
        return String::new();
    }
    let center = center.min(spans.len() - 1);
    let from = center.saturating_sub(CONTEXT_LINES);
    let to = (center + CONTEXT_LINES).min(spans.len() - 1);

    let mut rendered = Vec::new();
    for (offset, (start, end, _)) in spans[from..=to].iter().enumerate() {
        rendered.push(format!("{}: {}", from + offset + 1, &content[*start..*end]));
    }
    rendered.join("\n")
}
