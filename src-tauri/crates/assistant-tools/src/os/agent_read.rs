use async_trait::async_trait;
use encoding_rs::Encoding;
use serde_json::Value;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::context::ToolUseContext;
use crate::os::output::{truncate_output, OutputLimits, TruncatedOutput};
use crate::os::path;
use crate::os::text_encoding::{
    detect_and_decode, parse_optional_encoding, DecodedText, EncodingSource,
};
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::ToolResult;
use crate::storage::{ReadReceipt, ReadReceiptKey, ReadReceiptStore};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "read";

/// 行号前缀的列宽。`edit` 档 2 按同一常量剥离前缀，改动会同时影响两侧。
pub const LINE_NUMBER_WIDTH: usize = 6;
/// 行号与正文之间的分隔符。`edit` 档 2 按同一常量剥离前缀。
pub const LINE_NUMBER_SEPARATOR: char = '\t';
/// 单行字符上限，超出即截断并追加 [`LINE_TRUNCATED_MARKER`]。
pub const MAX_LINE_CHARS: usize = 2000;
/// 长行截断标记（前置一个空格）。`edit` 档 2 / 档 4 依赖此字面量。
pub const LINE_TRUNCATED_MARKER: &str = " [truncated]";

/// 单次读取的字节上限。
pub const DEFAULT_MAX_BYTES: usize = 1024 * 1024;
/// 单次读取的行数上限。
pub const DEFAULT_MAX_LINES: usize = 2000;

/// 读取回执的修订号：文件长度 + 修改时间（纳秒）。
///
/// 不做整文件哈希：哈希对大文件是纯开销，而回执只需要判断内容「是否可能已变」。
/// 长度与 mtime 都不变的「删除后重建」（保时间戳的复制 / 解包）由
/// `ReadReceiptStore::invalidate_path` 兜底，不依赖修订号。
pub fn file_revision(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("failed to stat '{}': {}", path.display(), error))?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("failed to read mtime of '{}': {}", path.display(), error))?;
    let nanos = modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    Ok(format!("{}:{}", metadata.len(), nanos))
}

pub struct AgentReadTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentReadTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentReadTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Read a text file with line numbers. Output lines are formatted as \
             `{line_number:>6}\\t{line}`; single lines longer than 2000 characters are cut and \
             marked ` [truncated]`. Reads up to 2000 lines / 1MB per call; use `offset` (1-based, \
             0 is treated as 1) and `limit` to page through larger files. Binary files are \
             rejected. Non-UTF-8 files are decoded via BOM detection, via the `encoding` label, \
             or by a confident guess. Re-reading the same range of an unchanged file in the same \
             session returns only a notice instead of the content. Does not follow `.gitignore`."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to read" },
                "offset": { "type": "integer", "description": "Start line (1-based; 0 is treated as 1)" },
                "limit": { "type": "integer", "description": "Max lines to read (must be >= 1)" },
                "encoding": {
                    "type": "string",
                    "description": "Optional encoding label (e.g. gbk); omit to auto-detect, BOM wins"
                }
            },
            "required": ["path"]
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
            read_only: true,
            destructive: false,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        match ReadRequest::parse(input) {
            Ok(request) => match path::resolve(&request.path, context) {
                Ok(_) => ValidationResult::ok(),
                Err(error) => ValidationResult::invalid(error),
            },
            Err(error) => ValidationResult::invalid(error),
        }
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match ReadRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let resolve_context = context.clone();
        let raw_path = request.path.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve(&raw_path, &resolve_context)?;
            let metadata = std::fs::metadata(&resolved)
                .map_err(|error| format!("failed to stat '{}': {}", resolved.display(), error))?;
            if metadata.is_dir() {
                return Err(format!(
                    "'{}' is a directory; use ls to list its entries",
                    resolved.display()
                ));
            }
            if !metadata.is_file() {
                return Err(format!("'{}' is not a regular file", resolved.display()));
            }
            let revision = file_revision(&resolved)?;
            Ok::<(PathBuf, String), String>((resolved, revision))
        })
        .await;

        let (resolved, revision) = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("read task failed: {}", error)),
        };

        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: resolved.display().to_string(),
            start_line: request.offset_specified.then_some(request.offset),
            end_line: request.limit.map(|limit| request.offset + limit as u64 - 1),
        };

        if let Some(receipt) = self.receipts.lookup(&key).await {
            if receipt.revision == revision {
                return Ok(ToolResult::ok_with_assistant(
                    serde_json::json!({
                        "path": resolved.display().to_string(),
                        "revision": revision,
                        "deduplicated": true
                    }),
                    format!(
                        "Already returned in this session and unchanged since revision {}. \
                         Use offset/limit to read a different range.",
                        revision
                    ),
                ));
            }
        }

        let read_path = resolved.clone();
        let offset = request.offset;
        let limit = request.limit;
        let encoding = request.encoding;
        let read =
            tokio::task::spawn_blocking(move || read_range(&read_path, offset, limit, encoding))
                .await;

        let (output, total_lines, decoded) = match read {
            Ok(Ok(read)) => read,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("read task failed: {}", error)),
        };

        let start_line = request.offset as usize;
        let returned_lines = output.returned_lines;
        let end_line = (returned_lines > 0).then(|| start_line + returned_lines - 1);

        let assistant_text = if returned_lines > 0 {
            output.text.clone()
        } else if total_lines == 0 {
            String::new()
        } else {
            format!(
                "No lines in range: file has {} lines, requested start line {}.",
                total_lines, start_line
            )
        };
        let assistant_text = match encoding_footer(&decoded) {
            Some(footer) => append_footer(&assistant_text, &footer),
            None => assistant_text,
        };

        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision,
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: decoded.encoding_name().to_string(),
                    has_bom: decoded.has_bom,
                },
            )
            .await;

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": resolved.display().to_string(),
                "startLine": start_line,
                "endLine": end_line,
                "totalLines": total_lines,
                "truncated": output.truncated,
                "deduplicated": false
            }),
            assistant_text,
        ))
    }
}

struct ReadRequest {
    path: String,
    offset: u64,
    limit: Option<usize>,
    offset_specified: bool,
    encoding: Option<&'static Encoding>,
}

impl ReadRequest {
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

        let offset = optional_u64(object, "offset")?;
        let limit = match optional_u64(object, "limit")? {
            Some(0) => return Err("'limit' must be >= 1".to_string()),
            Some(value) => Some(value as usize),
            None => None,
        };
        let encoding = parse_optional_encoding(object)?;

        Ok(Self {
            path,
            offset: offset.unwrap_or(1).max(1),
            limit,
            offset_specified: offset.is_some(),
            encoding,
        })
    }
}

fn optional_u64(object: &serde_json::Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("'{}' must be a non-negative integer", key)),
    }
}

/// 同步读取、解码与渲染，必须在 `spawn_blocking` 内调用。
fn read_range(
    path: &Path,
    offset: u64,
    limit: Option<usize>,
    encoding: Option<&'static Encoding>,
) -> Result<(TruncatedOutput, usize, DecodedText), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;

    let decoded = detect_and_decode(&bytes, encoding)
        .map_err(|error| format!("'{}': {}", path.display(), error))?;

    let text = decoded.text.as_str();
    let total_lines = text.lines().count();
    let start_index = (offset.saturating_sub(1) as usize).min(total_lines);
    let end_index = match limit {
        Some(limit) => start_index.saturating_add(limit).min(total_lines),
        None => total_lines,
    };

    let mut rendered = String::new();
    for (index, line) in text.lines().enumerate().take(end_index).skip(start_index) {
        let number = index + 1;
        let rendered_line = truncate_line(line);
        let _ = writeln!(
            rendered,
            "{:>width$}{}{}",
            number,
            LINE_NUMBER_SEPARATOR,
            rendered_line,
            width = LINE_NUMBER_WIDTH
        );
    }

    let limits = OutputLimits {
        max_bytes: DEFAULT_MAX_BYTES,
        max_lines: DEFAULT_MAX_LINES,
        preview_bytes: 0,
    };
    Ok((truncate_output(&rendered, &limits), total_lines, decoded))
}

/// 编码 footer（上级 §14.4）：UTF-8 无 BOM 时静默，其余带来源后缀。
fn encoding_footer(decoded: &DecodedText) -> Option<String> {
    if decoded.source == EncodingSource::Utf8 && !decoded.has_bom {
        return None;
    }
    let label = decoded.encoding_name();
    Some(match decoded.source.suffix() {
        Some(suffix) => format!("[encoding: {} ({})]", label, suffix),
        None => format!("[encoding: {}]", label),
    })
}

/// 追加 footer 并保证它独占最后一行：正文已以换行结束时不再补空行。
fn append_footer(text: &str, footer: &str) -> String {
    if text.is_empty() {
        return footer.to_string();
    }
    if text.ends_with('\n') {
        format!("{}{}", text, footer)
    } else {
        format!("{}\n{}", text, footer)
    }
}

fn truncate_line(line: &str) -> String {
    if line.chars().count() <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let mut truncated: String = line.chars().take(MAX_LINE_CHARS).collect();
    truncated.push_str(LINE_TRUNCATED_MARKER);
    truncated
}
