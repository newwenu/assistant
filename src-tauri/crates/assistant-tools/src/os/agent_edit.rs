use async_trait::async_trait;
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::agent_read::{file_revision, LINE_NUMBER_SEPARATOR, LINE_TRUNCATED_MARKER};
use crate::os::path;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::{ReadReceipt, ReadReceiptKey, ReadReceiptStore};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "edit";

/// 歧义错误里最多展示的上下文段数。
pub const MAX_MATCH_CONTEXTS: usize = 5;

/// 上下文段 / 最近片段的前后行数。
pub const CONTEXT_LINES: usize = 2;

/// 归一化匹配尝试的 tab 宽度：4 空格优先，其次 2 空格。
pub const TAB_WIDTHS: [usize; 2] = [4, 2];

/// diff 与预演的每侧最大行数。
pub const MAX_DIFF_LINES: usize = 20;

pub struct AgentEditTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentEditTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentEditTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Replace an exact fragment of an existing UTF-8 text file. `old_string` must be \
             copied from the latest read output (without the line-number prefix) and match \
             exactly once unless `replace_all = true`. Matching degrades progressively \
             (exact, stripped line prefixes, trailing newline, trailing whitespace, leading \
             whitespace, normalized) and stops at the first pass that hits; multiple hits in \
             that pass are reported as ambiguous instead of picking the first. The file must \
             have been read (or written) in this session and must not have changed since. \
             Writes are atomic (temp file + fsync + rename); CRLF files stay CRLF."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to edit" },
                "old_string": {
                    "type": "string",
                    "description": "Exact current text to replace, copied from the latest read output without the line-number prefix"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text; may be empty to delete the fragment. Keep the indentation style of old_string"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace every match instead of requiring a unique match (default false)"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::BOTH
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: false,
            destructive: false,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        let request = match EditRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if let Some(error) = rejected_arguments(&request) {
            return ValidationResult::invalid(error);
        }
        match path::resolve(&request.path, context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = EditRequest::parse(input)?;
        let resolved = path::resolve(&request.path, context)?;
        Ok(vec![PermissionIntent::new(
            "edit",
            resolved.display().to_string(),
        )])
    }

    async fn preview_for_approval(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        let request = EditRequest::parse(input).ok()?;
        let preview_context = context.clone();
        tokio::task::spawn_blocking(move || build_preview(&request, &preview_context))
            .await
            .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match EditRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        if let Some(error) = rejected_arguments(&request) {
            return Ok(ToolResult::err(error));
        }

        let probe_context = context.clone();
        let probe_request = request.clone();
        let probed =
            tokio::task::spawn_blocking(move || probe_target(&probe_request, &probe_context)).await;
        let (resolved, revision) = match probed {
            Ok(Ok(probed)) => probed,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("edit task failed: {}", error)),
        };

        let path_text = resolved.display().to_string();
        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: path_text.clone(),
            start_line: None,
            end_line: None,
        };
        match self.receipts.lookup(&key).await {
            None => {
                return Ok(ToolResult::err(format!(
                    "'{}' has not been read in this session; call read on the whole file before \
                     editing (blind edits can overwrite changes made outside this session)",
                    path_text
                )))
            }
            Some(receipt) if receipt.revision != revision => {
                return Ok(ToolResult::err(format!(
                    "'{}' changed after it was last read in this session (revision {} -> {}); \
                     call read again before editing",
                    path_text, receipt.revision, revision
                )))
            }
            Some(_) => {}
        }

        let edit_request = request.clone();
        let edited = tokio::task::spawn_blocking(move || apply_edit(&resolved, &edit_request)).await;
        let (outcome, revision) = match edited {
            Ok(Ok(edited)) => edited,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("edit task failed: {}", error)),
        };

        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision: revision.clone(),
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                },
            )
            .await;

        let assistant_text = format!(
            "Edited {}: {} replacement(s) at line {} (lines {} -> {}).\n{}",
            path_text,
            outcome.match_count,
            outcome.start_line,
            outcome.old_end_line,
            outcome.new_end_line,
            outcome.diff
        );

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "startLine": outcome.start_line,
                "oldEndLine": outcome.old_end_line,
                "newEndLine": outcome.new_end_line,
                "matchCount": outcome.match_count,
                "tier": outcome.tier,
                "diff": outcome.diff,
                "revision": revision
            }),
            assistant_text,
        ))
    }
}

#[derive(Clone)]
struct EditRequest {
    path: String,
    old_string: String,
    new_string: String,
    replace_all: bool,
}

impl EditRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = object
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing or empty 'path' field".to_string())?
            .to_string();

        let old_string = object
            .get("old_string")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing or non-string 'old_string' field".to_string())?
            .to_string();

        let new_string = object
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing or non-string 'new_string' field".to_string())?
            .to_string();

        let replace_all = match object.get("replace_all") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'replace_all' must be a boolean".to_string()),
        };

        Ok(Self {
            path,
            old_string,
            new_string,
            replace_all,
        })
    }
}

/// 档 0：无意义写回的前置拒绝。纯参数判定，不触碰文件系统。
fn rejected_arguments(request: &EditRequest) -> Option<String> {
    if request.old_string.is_empty() {
        return Some("'old_string' must not be empty".to_string());
    }
    if request.old_string == request.new_string {
        return Some("'old_string' and 'new_string' are identical; nothing to change".to_string());
    }
    None
}

/// 一次编辑的结果。行号均以替换前的文件为准，`match_count` 为实际替换次数。
struct EditOutcome {
    start_line: usize,
    old_end_line: usize,
    new_end_line: usize,
    match_count: usize,
    tier: &'static str,
    diff: String,
}

/// 一次命中的定位结果。`hits` 的偏移全部落在 LF 归一后的文本上。
struct Located {
    hits: Vec<(usize, usize)>,
    replacement: String,
    tier: &'static str,
    /// 行级匹配（档 4 / 5）的命中片段包含行尾换行，替换时需补齐。
    line_based: bool,
}

/// 同步探测目标：解析路径、确认是普通文件、取当前修订号。必须在 `spawn_blocking` 内调用。
fn probe_target(
    request: &EditRequest,
    context: &ToolUseContext,
) -> Result<(PathBuf, String), String> {
    let resolved = path::resolve(&request.path, context)?;
    let metadata = match std::fs::metadata(&resolved) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "'{}' does not exist; create it with the write tool first",
                resolved.display()
            ))
        }
        Err(error) => {
            return Err(format!(
                "cannot access '{}': {}",
                resolved.display(),
                error
            ))
        }
    };
    if metadata.is_dir() {
        return Err(format!(
            "'{}' is a directory; edit works on files only",
            resolved.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("'{}' is not a regular file", resolved.display()));
    }
    let revision = file_revision(&resolved)?;
    Ok((resolved, revision))
}

/// 同步编辑并写回。必须在 `spawn_blocking` 内调用。
fn apply_edit(path: &Path, request: &EditRequest) -> Result<(EditOutcome, String), String> {
    let path_text = path.display().to_string();
    let raw = std::fs::read(path)
        .map_err(|error| format!("cannot read '{}': {}", path_text, error))?;
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "'{}' is not valid UTF-8; edit only handles UTF-8 text files",
            path_text
        )
    })?;

    let uses_crlf = text.contains("\r\n");
    let content = if uses_crlf {
        text.replace("\r\n", "\n")
    } else {
        text
    };

    let located = match locate(&content, &request.old_string, &request.new_string) {
        Some(located) => located,
        None => return Err(not_found_message(&path_text, &content, &request.old_string)),
    };

    if located.hits.len() > 1 && !request.replace_all {
        return Err(ambiguous_message(&path_text, &content, &located.hits));
    }

    let changed = located.hits.iter().any(|hit| {
        content[hit.0..hit.1] != effective_replacement(&content, *hit, &located)
    });
    if !changed {
        let start_line = line_number_at(&content, located.hits[0].0);
        return Err(format!(
            "'old_string' at line {} of '{}' already equals 'new_string'; nothing to change",
            start_line, path_text
        ));
    }

    let updated = apply_hits(&content, &located.hits, &located);
    let payload = if uses_crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    persist_atomically(path, payload.as_bytes())?;
    let revision = file_revision(path)?;

    let first = located.hits[0];
    let start_line = line_number_at(&content, first.0);
    let replacement = effective_replacement(&content, first, &located);
    let old_lines = content[first.0..first.1].lines().count().max(1);
    let new_lines = replacement.lines().count().max(1);

    Ok((
        EditOutcome {
            start_line,
            old_end_line: start_line + old_lines - 1,
            new_end_line: start_line + new_lines - 1,
            match_count: located.hits.len(),
            tier: located.tier,
            diff: diff_text(&content[first.0..first.1], &replacement),
        },
        revision,
    ))
}

/// 渐进匹配主流程：档 1 → 2 → 3 → 4 → 5 → 6，命中即停。
///
/// 每档都先拿到该档的全部命中，再交给调用方判定「唯一 / 歧义」，
/// 因此不存在「先取首个再回退」的路径。全部档位失败返回 `None`。
fn locate(content: &str, old: &str, new: &str) -> Option<Located> {
    let exact = substring_hits(content, old);
    if !exact.is_empty() {
        return Some(Located {
            hits: exact,
            replacement: new.to_string(),
            tier: "exact",
            line_based: false,
        });
    }

    if let Some(stripped) = strip_read_line_prefixes(old) {
        let hits = substring_hits(content, &stripped);
        if !hits.is_empty() {
            return Some(Located {
                hits,
                replacement: new.to_string(),
                tier: "prefix-stripped",
                line_based: false,
            });
        }
    }

    if !old.ends_with('\n') {
        let mut candidate = old.to_string();
        candidate.push('\n');
        let hits = substring_hits(content, &candidate);
        if !hits.is_empty() {
            let mut replacement = new.to_string();
            if !replacement.ends_with('\n') {
                replacement.push('\n');
            }
            return Some(Located {
                hits,
                replacement,
                tier: "trailing-newline",
                line_based: false,
            });
        }
    }

    let trimmed_end = line_hits(content, old, LineMode::TrimEnd);
    if !trimmed_end.is_empty() {
        return Some(Located {
            hits: trimmed_end,
            replacement: new.to_string(),
            tier: "trim-end",
            line_based: true,
        });
    }

    let trimmed = line_hits(content, old, LineMode::Trim);
    if !trimmed.is_empty() {
        return Some(Located {
            hits: trimmed,
            replacement: new.to_string(),
            tier: "trim",
            line_based: true,
        });
    }

    let normalized = normalized_hits(content, old);
    if !normalized.is_empty() {
        return Some(Located {
            hits: normalized,
            replacement: new.to_string(),
            tier: "normalized",
            line_based: false,
        });
    }

    None
}

fn effective_replacement(content: &str, hit: (usize, usize), located: &Located) -> String {
    let mut replacement = located.replacement.clone();
    if located.line_based && content[hit.0..hit.1].ends_with('\n') && !replacement.ends_with('\n')
    {
        replacement.push('\n');
    }
    replacement
}

fn apply_hits(content: &str, hits: &[(usize, usize)], located: &Located) -> String {
    let mut updated = content.to_string();
    for hit in hits.iter().rev() {
        let replacement = effective_replacement(content, *hit, located);
        updated.replace_range(hit.0..hit.1, &replacement);
    }
    updated
}

fn substring_hits(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    haystack
        .match_indices(needle)
        .map(|(start, matched)| (start, start + matched.len()))
        .collect()
}

/// 档 2 的行号前缀剥离：仅当每个非空行都形如「空格 + 数字 + tab 或 `→`」时才生效。
fn strip_read_line_prefixes(old: &str) -> Option<String> {
    let mut stripped = String::with_capacity(old.len());
    let mut saw_content = false;
    for (index, line) in old.split('\n').enumerate() {
        if index > 0 {
            stripped.push('\n');
        }
        if line.trim().is_empty() {
            stripped.push_str(line);
            continue;
        }
        let rest = strip_one_line_prefix(line)?;
        saw_content = true;
        stripped.push_str(rest);
    }
    saw_content.then_some(stripped)
}

fn strip_one_line_prefix(line: &str) -> Option<&str> {
    let without_indent = line.trim_start_matches(' ');
    let digits = without_indent
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .count();
    if digits == 0 {
        return None;
    }
    let rest = &without_indent[digits..];
    if let Some(after_separator) = rest.strip_prefix(LINE_NUMBER_SEPARATOR) {
        return Some(after_separator);
    }
    let after_arrow = rest.trim_start_matches(' ').strip_prefix('→')?;
    Some(after_arrow.strip_prefix(' ').unwrap_or(after_arrow))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineMode {
    TrimEnd,
    Trim,
}

fn line_hits(content: &str, pattern: &str, mode: LineMode) -> Vec<(usize, usize)> {
    let spans = line_spans(content);
    let pattern_lines = pattern_lines(pattern);
    if spans.is_empty() || pattern_lines.is_empty() || pattern_lines.len() > spans.len() {
        return Vec::new();
    }

    let normalize = |line: &str| match mode {
        LineMode::TrimEnd => line.trim_end().to_string(),
        LineMode::Trim => line.trim().to_string(),
    };
    let needle: Vec<String> = pattern_lines
        .iter()
        .map(|line| normalize(line))
        .collect();

    let mut hits = Vec::new();
    for start in 0..=(spans.len() - needle.len()) {
        let matched = (0..needle.len()).all(|offset| {
            let (line_start, line_end, _) = spans[start + offset];
            normalize(&content[line_start..line_end]) == needle[offset]
        });
        if matched {
            let first = spans[start];
            let last = spans[start + needle.len() - 1];
            hits.push((first.0, last.2));
        }
    }
    hits
}

fn pattern_lines(pattern: &str) -> Vec<&str> {
    let trimmed = pattern.strip_suffix('\n').unwrap_or(pattern);
    trimmed.split('\n').collect()
}

/// 行视图：`(内容起点, 内容终点（不含 \n）, 下一行起点)`。
fn line_spans(text: &str) -> Vec<(usize, usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            spans.push((start, index, index + 1));
            start = index + 1;
        }
    }
    if start < text.len() {
        spans.push((start, text.len(), text.len()));
    }
    spans
}

fn line_index_at(text: &str, byte: usize) -> usize {
    text[..byte].bytes().filter(|byte| *byte == b'\n').count()
}

fn line_number_at(text: &str, byte: usize) -> usize {
    line_index_at(text, byte) + 1
}

/// 归一化视图：逐字符 1:1 折叠，`orig_offsets` 把折叠视图的字符下标映射回原文。
struct FoldedView {
    text: String,
    orig_offsets: Vec<usize>,
    fold_offsets: Vec<usize>,
}

fn fold_char(ch: char) -> char {
    match ch {
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201C}' | '\u{201D}' => '"',
        other => other,
    }
}

fn fold_view(text: &str) -> FoldedView {
    let mut folded = String::with_capacity(text.len());
    let mut orig_offsets = Vec::new();
    let mut fold_offsets = Vec::new();
    for (index, ch) in text.char_indices() {
        orig_offsets.push(index);
        fold_offsets.push(folded.len());
        folded.push(fold_char(ch));
    }
    orig_offsets.push(text.len());
    fold_offsets.push(folded.len());
    FoldedView {
        text: folded,
        orig_offsets,
        fold_offsets,
    }
}

impl FoldedView {
    fn char_index(&self, byte: usize) -> usize {
        self.fold_offsets.partition_point(|offset| *offset < byte)
    }
}

fn normalized_hits(content: &str, old: &str) -> Vec<(usize, usize)> {
    let view = fold_view(content);

    let mut candidates = Vec::new();
    candidates.push(fold_quotes(old));
    for width in TAB_WIDTHS {
        candidates.push(fold_quotes(&convert_leading_spaces_to_tabs(old, width)));
        candidates.push(fold_quotes(&convert_leading_tabs_to_spaces(old, width)));
    }

    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        let hits: Vec<(usize, usize)> = substring_hits(&view.text, &candidate)
            .into_iter()
            .map(|(start, end)| {
                (
                    view.orig_offsets[view.char_index(start)],
                    view.orig_offsets[view.char_index(end)],
                )
            })
            .collect();
        if !hits.is_empty() {
            return hits;
        }
    }
    Vec::new()
}

fn fold_quotes(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

fn leading_whitespace_len(line: &str) -> usize {
    line.len() - line.trim_start_matches(&[' ', '\t'][..]).len()
}

fn convert_leading_tabs_to_spaces(text: &str, width: usize) -> String {
    let converted: Vec<String> = text
        .split('\n')
        .map(|line| {
            let prefix_len = leading_whitespace_len(line);
            let prefix = &line[..prefix_len];
            if !prefix.contains('\t') {
                return line.to_string();
            }
            format!("{}{}", prefix.replace('\t', &" ".repeat(width)), &line[prefix_len..])
        })
        .collect();
    converted.join("\n")
}

fn convert_leading_spaces_to_tabs(text: &str, width: usize) -> String {
    let converted: Vec<String> = text
        .split('\n')
        .map(|line| {
            let prefix_len = leading_whitespace_len(line);
            let prefix = &line[..prefix_len];
            if width == 0 || prefix.is_empty() || prefix.contains('\t') || prefix_len % width != 0 {
                return line.to_string();
            }
            format!("{}{}", "\t".repeat(prefix_len / width), &line[prefix_len..])
        })
        .collect();
    converted.join("\n")
}

fn persist_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", path.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create temp file in '{}' failed: {}", parent.display(), error))?;
    temp.write_all(bytes)
        .map_err(|error| format!("write temp file failed: {}", error))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("sync temp file failed: {}", error))?;
    temp.persist(path)
        .map_err(|error| format!("persist '{}' failed: {}", path.display(), error))?;
    Ok(())
}

fn diff_text(old: &str, new: &str) -> String {
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

fn build_preview(
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

fn not_found_message(path_text: &str, content: &str, old: &str) -> String {
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
    if let Some(snippet) = nearest_snippet(content, old) {
        message.push_str("\nNearest text in the file:\n");
        message.push_str(&snippet);
    }
    message
}

fn ambiguous_message(path_text: &str, content: &str, hits: &[(usize, usize)]) -> String {
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
        if best.is_none_or(|(best_score, _)| score > best_score) {
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
    for index in from..=to {
        let (start, end, _) = spans[index];
        rendered.push(format!("{}: {}", index + 1, &content[start..end]));
    }
    rendered.join("\n")
}