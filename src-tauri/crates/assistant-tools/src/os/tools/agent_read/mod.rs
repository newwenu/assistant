use async_trait::async_trait;
use encoding_rs::Encoding;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::context::ToolUseContext;
use crate::os::footer;
use crate::os::output::human_bytes;
use crate::os::path;
use crate::os::text_encoding::{parse_optional_encoding, DecodedInfo, EncodingSource};
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::runtime::args::optional_u64;
use crate::storage::{
    ReadReceipt, ReadReceiptKey, ReadReceiptStore, DEFAULT_LINE_WINDOW_MAX_CHARS,
};

pub(crate) mod window;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "read";

/// 行号前缀的列宽。`edit` 档 2 按同一常量剥离前缀，改动会同时影响两侧。
pub const LINE_NUMBER_WIDTH: usize = 6;
/// 行号与正文之间的分隔符。`edit` 档 2 按同一常量剥离前缀。
pub const LINE_NUMBER_SEPARATOR: char = '\t';
/// 单行字符上限，超出即截断并追加 [`LINE_TRUNCATED_MARKER`]。
///
/// 这是**硬上限**而不是可调参数：`read` 只报告「有多少行被切」，
/// 行内翻页由 I58 的 `super_read` 承接（I57 §3.2）。
pub const MAX_LINE_CHARS: usize = DEFAULT_LINE_WINDOW_MAX_CHARS;
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
             marked ` [truncated]`, and the count of such lines is reported as `truncatedLines` \
             plus a `[long-lines: ...]` footer, so a cut line is never silently mistaken for a \
             complete one. Reads up to 2000 lines / 1MB per call for files of any size; \
             use `offset` (1-based, 0 is treated as 1) and `limit` to page through larger files. \
             `totalLines` is null when the file is too large to count to the end in one call. \
             Binary files are rejected. Non-UTF-8 files are decoded via BOM detection, via the \
             `encoding` label, or by a confident guess. Does not follow `.gitignore`. Tool \
             output files referenced by a `<persisted-output path=\"...\">` tag can be read with \
             this tool; the first such read asks the user for permission."
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
            Ok(request) => match path::resolve_readable(&request.path, context) {
                Ok(_) => ValidationResult::ok(),
                Err(error) => ValidationResult::invalid(error),
            },
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = ReadRequest::parse(input)?;
        let resolved = path::resolve_readable(&request.path, context)?;
        // 输出域落在工作区之外，必须走更严的独立动作，不能借 `read` 的默认 `Allow` 混过去。
        // `resource` 取命名空间**目录**：审批键覆盖本会话全部输出，读第 2、第 3 个文件不再弹框。
        match resolved.scope {
            path::ReadableScope::Workspace => Ok(vec![PermissionIntent::new(
                "read",
                resolved.path.display().to_string(),
            )]),
            path::ReadableScope::ToolOutput => {
                let root = context
                    .tool_output_root
                    .as_deref()
                    .ok_or_else(|| "tool output root is unavailable".to_string())?;
                Ok(vec![PermissionIntent::new(
                    "readToolOutput",
                    root.display().to_string(),
                )])
            }
        }
    }

    async fn preview_for_approval(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        let request = ReadRequest::parse(input).ok()?;
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            let resolved = path::resolve_readable(&request.path, &context).ok()?;
            // 工作区内读取由 `read` 的默认 `Allow` 覆盖，不会走到审批卡片；
            // 只有跨出边界的输出域读取才需要量级摘要。
            if resolved.scope != path::ReadableScope::ToolOutput {
                return None;
            }
            let bytes = std::fs::metadata(&resolved.path).ok()?.len();
            // 文件名只取末段：卡片不暴露本机绝对路径（I52 §5.11.4(5)）。
            let file_name = resolved.path.file_name()?.to_string_lossy().to_string();
            let first = request.offset;
            let line_cap = request
                .limit
                .map_or(DEFAULT_MAX_LINES, |value| value.min(DEFAULT_MAX_LINES));
            let last = first.saturating_add(line_cap as u64 - 1);
            Some(ApprovalPreview {
                summary: format!(
                    "读本会话输出 {} · 第 {}–{} 行 · {}",
                    file_name,
                    first,
                    last,
                    human_bytes(bytes as usize)
                ),
                truncated: false,
                estimated: false,
            })
        })
        .await
        .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match ReadRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let resolve_context = context.clone();
        let raw_path = request.path.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve_readable(&raw_path, &resolve_context)?.path;
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
        };

        let read_path = resolved.clone();
        let offset = request.offset;
        let limit = request.limit;
        let encoding = request.encoding;
        let scanned = tokio::task::spawn_blocking(move || {
            window::scan(&window::WindowRequest {
                path: &read_path,
                offset,
                limit,
                encoding,
            })
        })
        .await;

        let outcome = match scanned {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("read task failed: {}", error)),
        };

        let start_line = request.offset;
        let returned_lines = outcome.returned_lines;
        let end_line = (returned_lines > 0).then(|| start_line + returned_lines as u64 - 1);

        let mut assistant_text = if returned_lines > 0 {
            outcome.rendered.clone()
        } else if outcome.total_lines == Some(0) {
            String::new()
        } else {
            match outcome.total_lines {
                Some(total) => format!(
                    "No lines in range: file has {} lines, requested start line {}.",
                    total, start_line
                ),
                None => format!(
                    "No lines in range: no content at or after line {}.",
                    start_line
                ),
            }
        };
        if outcome.truncated {
            assistant_text.push_str(&truncation_notice(&outcome));
        }
        if let Some(footer) = long_lines_footer(&outcome) {
            assistant_text = append_footer(&assistant_text, &footer);
        }
        if let Some(footer) = encoding_footer(&outcome.info) {
            assistant_text = append_footer(&assistant_text, &footer);
        }
        // 能力升级提示（I58 §3.6）：普通工具优先，只在它表达不了、且超级工具本轮可用时出现。
        if let Some(hint) = capability_hint(&outcome, context) {
            assistant_text = append_footer(&assistant_text, &hint);
        }

        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision,
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: outcome.info.encoding_name().to_string(),
                    has_bom: outcome.info.has_bom,
                    truncated_lines: outcome.truncated_lines,
                    line_window_max_chars: MAX_LINE_CHARS,
                },
            )
            .await;

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": resolved.display().to_string(),
                "startLine": start_line,
                "endLine": end_line,
                "totalLines": outcome.total_lines,
                "truncated": outcome.truncated,
                "truncatedLines": outcome.truncated_lines
            }),
            assistant_text,
        ))
    }
}

struct ReadRequest {
    path: String,
    offset: u64,
    limit: Option<usize>,
    encoding: Option<&'static Encoding>,
}

impl ReadRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

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
            encoding,
        })
    }
}

/// 截断提示（结果正文尾部，footer 之前）：四态（I57 §6.2）。
///
/// 归因轴只有两个：`total_lines` 是否有值（= 是否读到 EOF），以及 `window_full`
/// （= 窗口是否已满；`false` 表示「窗口未满也停」，即预算耗尽）。S1 / S2 与改动前
/// **逐字节一致**，S3 / S4 是本阶段新增的两态。
/// 禁用「more content follows」一类既无总数也无未知标记的写法（I46 compact §2.2 D2）。
fn truncation_notice(outcome: &window::WindowOutcome) -> String {
    if outcome.returned_lines == 0 {
        // S4：无输出行 + 预算耗尽。当前实现下不可达（首个渲染行不受预算限制，
        // 而停止判定只在渲染过至少一行之后才可能成立），保留分支以免四态缺口。
        return footer::render(
            footer::TRUNCATED,
            "no lines rendered; scan budget exhausted",
        );
    }
    let detail = match outcome.total_lines {
        Some(total) => format!(
            "showing {} of {} lines, {} of {} bytes",
            outcome.returned_lines, total, outcome.rendered_bytes, outcome.total_rendered_bytes
        ),
        None if outcome.window_full => format!(
            "showing {} lines, {} bytes; total unknown",
            outcome.returned_lines, outcome.rendered_bytes
        ),
        None => format!(
            "scan budget exhausted after {} bytes; showing {} lines, total unknown",
            outcome.scanned_bytes, outcome.returned_lines
        ),
    };
    footer::render(footer::TRUNCATED, detail)
}

/// 长行 footer（I57 §3.3）：只在本次输出确有被切的行时出现。
///
/// `n` 与 `data.truncatedLines` 同源（同一个计数器，不二次统计）；
/// `width` 是硬上限 `MAX_LINE_CHARS`，不是可调参数。
fn long_lines_footer(outcome: &window::WindowOutcome) -> Option<String> {
    (outcome.truncated_lines > 0).then(|| {
        footer::render(
            footer::LONG_LINES,
            format!(
                "{} lines cut at {} chars",
                outcome.truncated_lines, MAX_LINE_CHARS
            ),
        )
    })
}

/// 编码 footer（上级 §14.4）：UTF-8 无 BOM 时静默，其余带来源后缀。
fn encoding_footer(info: &DecodedInfo) -> Option<String> {
    if info.source == EncodingSource::Utf8 && !info.has_bom {
        return None;
    }
    let label = info.encoding_name();
    Some(match info.source.suffix() {
        Some(suffix) => footer::render(footer::ENCODING, format!("{} ({})", label, suffix)),
        None => footer::render(footer::ENCODING, label),
    })
}

/// 追加 footer 并保证它独占最后一行：正文已以换行结束时不再补空行。
fn append_footer(text: &str, line: &str) -> String {
    if text.is_empty() {
        return line.to_string();
    }
    if text.ends_with('\n') {
        format!("{}{}", text, line)
    } else {
        format!("{}\n{}", text, line)
    }
}

/// 能力升级提示（I58 §3.6）：普通工具优先，只在它确实表达不了时出现。
///
/// 两个触发条件都对应 `super_read` 能做而 `read` 做不到的事：
/// - 有行被切：`read` 无法进入行内，`super_read` 的 `column` 可以；
/// - `totalLines` 未知：`read` 的扫描撞上预算，`super_read` 的总行数恒精确。
///
/// 门控在本轮工具集里有 `request_tool`（唯一的申请入口），而不是 `super_read`——
/// 后者按设计**不在**本轮工具集里（`brief` 档，模型申请后才追加）。建议一个调不到的
/// 工具比不提示更糟，所以提示里给的是「怎么申请」，不是「直接调用」。
fn capability_hint(outcome: &window::WindowOutcome, context: &ToolUseContext) -> Option<String> {
    if !context
        .available_tool_names
        .as_ref()
        .is_some_and(|names| names.iter().any(|name| name == "request_tool"))
    {
        return None;
    }
    if outcome.truncated_lines > 0 {
        return Some(footer::render(
            footer::HINT,
            "long lines were cut; request_tool(detail = \"enable\", tools = [\"super_read\"]) \
             enables in-line reads via `column`",
        ));
    }
    outcome.total_lines.is_none().then(|| {
        footer::render(
            footer::HINT,
            "totalLines is unknown; request_tool(detail = \"enable\", tools = [\"super_read\"]) \
             reports an exact `totalLines`",
        )
    })
}
