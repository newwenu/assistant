//! `super_edit`：按行地址替换字节区间（I58 §3.3）。
//!
//! 与 `edit` 的分工是**契约对齐、实现独立**（I58 §0.3）：审批键沿用 `edit`，回执字段同构；
//! 但定位只按行地址，不做子串匹配，写回只改目标区间。索引、行起点自证、编码探测
//! 全部来自 B 私有公共层 `super_index`（§7.2 / G18），与 `super_read` 共用同一份判据。
//!
//! 与 `edit` 的关键差别：
//! - 不查读取回执（没有子串匹配，盲写风险由 `expected` 逐字节校验封死：F7 / F8 / F10）；
//! - 不整文件驻留：`head + replacement + tail` 三段流式写，区间外字节逐字节原样保留；
//! - 闸门按区间计（`MAX_EDIT_RANGE_BYTES`），转码闸门只约束 `replacement` 段。

use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::context::ToolUseContext;
use crate::os::agent_read::{file_revision, LINE_NUMBER_SEPARATOR, LINE_NUMBER_WIDTH};
use crate::os::footer;
use crate::os::path;
use crate::os::text_encoding::{guess_encoding, DecodeError, EncodingSource};
use crate::os::tools::super_index::proof::{ColumnUnit, IndexError, MAX_COLUMN_CHARS};
use crate::os::tools::super_index::{cache, format};
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::runtime::args::{optional_bool, optional_u64, required_u64};
use crate::storage::{
    ReadReceipt, ReadReceiptKey, ReadReceiptStore, DEFAULT_LINE_WINDOW_MAX_CHARS,
};

mod apply;
mod locate;

use cache::IndexState;
use locate::{Address, LocateError, Located};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "super_edit";

/// 猜编码时的采样上限：与 `super_read` 同量级，不为判定把大文件整个再读一遍。
const GUESS_SAMPLE_BYTES: usize = 1024 * 1024;
/// `expected` 不符时回显的现文本长度：够看出漂移方向，又不把输出撑爆。
const MISMATCH_EXCERPT_CHARS: usize = 200;
/// 回执与预览里最多回显的 diff 行数。
const MAX_DIFF_LINES: usize = 10;

/// 按行地址替换文本文件的一个字节区间（I58 §3.3）。
///
/// 索引是惰性资源：首次调用自行建索引（与 `super_read` 同一缓存与单飞），
/// 因此不要求先调用过 `super_read`。
pub struct SuperEditTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl SuperEditTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for SuperEditTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok("Replace a half-open range addressed by line and column: \
             `[startLine:startColumn, endLine:endColumn)`, 1-based, `endLine` inclusive and \
             `endColumn` exclusive. `endColumn = startColumn` is an empty range, i.e. a pure \
             insertion. Locating is positional: the range is found by line and column address, \
             so no unique anchor is needed; bytes outside it are copied through unchanged. \
             `expected` is mandatory whenever the range is non-empty — it must equal the current \
             text of that range byte for byte. The first call on a file pays a one-off indexing \
             cost."
            .to_string())
    }

    fn brief(&self) -> Option<&'static str> {
        Some(
            "Replace a text range addressed by line/column. Request it when `edit` cannot \
             express the change.",
        )
    }

    fn manual(&self) -> Option<&'static str> {
        Some(
            "Use this when `edit` cannot express the change: you know the exact line address \
             (for example from `super_read` or `grep`) but not a unique substring, or the \
             surrounding lines are too long to copy. `edit` stays the default for ordinary \
             string replacement.\n\n\
             Range semantics: `startLine`/`endLine` are 1-based and `endLine` is inclusive; \
             `startColumn`/`endColumn` are 1-based and the end column is exclusive, so \
             `endColumn = startColumn` is an empty range (pure insertion). `endColumn` defaults \
             to the end of `endLine`; `toEndOfLine` is the same thing spelled explicitly and is \
             mutually exclusive with `endColumn`. Columns count decoded characters by default \
             (`unit = \"char\"`, capped at 200000) or raw bytes of the line content \
             (`unit = \"byte\"`, must land on a character boundary).\n\n\
             `expected` is mandatory whenever the range is not empty: it must equal the current \
             text of that range byte for byte, after normalizing `\\r\\n` to `\\n`. Without it \
             a delete or overwrite could silently destroy text you never verified. Pure \
             insertions (empty range) may omit it.\n\n\
             What is guaranteed: bytes outside the range are preserved exactly, the BOM stays \
             where it is, and `replacement` is encoded in the file's own encoding — line \
             endings inside `replacement` follow the file's style (a CRLF file keeps CRLF). \
             The range itself is capped at 16 MiB and a transcoded `replacement` at 1 MiB. \
             Whole-file line-ending normalization is not performed; use `edit` when that is \
             what you need.\n\n\
             If the file changed between locating and writing, the call is refused rather than \
             applied blindly: re-run `super_read` to see the current text and retry.",
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to edit" },
                "startLine": {
                    "type": "integer",
                    "description": "First line of the range (1-based)"
                },
                "startColumn": {
                    "type": "integer",
                    "description": "In-line start position on startLine (1-based; 0 is treated as 1); `unit` decides the unit"
                },
                "endLine": {
                    "type": "integer",
                    "description": "Last line of the range, inclusive (defaults to startLine)"
                },
                "endColumn": {
                    "type": "integer",
                    "description": "In-line end position on endLine, exclusive (defaults to the end of that line); mutually exclusive with toEndOfLine"
                },
                "toEndOfLine": {
                    "type": "boolean",
                    "description": "End the range at the end of endLine; mutually exclusive with endColumn"
                },
                "unit": {
                    "type": "string",
                    "enum": ["char", "byte"],
                    "description": "Unit of the columns: decoded characters (default) or raw bytes of the line content"
                },
                "expected": {
                    "type": "string",
                    "description": "Current text of the range; required whenever the range is not empty"
                },
                "replacement": {
                    "type": "string",
                    "description": "Text written in place of the range; an empty string deletes it (defaults to \"\")"
                }
            },
            "required": ["path", "startLine"]
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    /// 跟随 `edit` 的勾选状态：用户不需要单独勾选它（I58 §3.6）。
    fn companion_of(&self) -> Option<&str> {
        Some("edit")
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::BOTH
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: false,
            destructive: true,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        match Request::parse(input) {
            Ok(request) => match request.path.resolve_write(context) {
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
        let request = Request::parse(input)?;
        let resolved = request.path.resolve_write(context)?;
        // 与 `edit` 同一审批键：写侧只认「编辑」这一件事，不因工具名分叉。
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
        let request = Request::parse(input).ok()?;
        let preview_context = context.clone();
        tokio::task::spawn_blocking(move || {
            let resolved = request.path.resolve_write(&preview_context).ok()?;
            let index_root = preview_context.index_root.clone()?;
            let metadata = std::fs::metadata(&resolved).ok()?;
            let revision = file_revision(&resolved).ok()?;
            let prepared = prepare(
                &index_root,
                &resolved,
                metadata.len(),
                &revision,
                &request.address,
            )
            .ok()?;
            let old_lines = line_count(&prepared.located.text);
            let new_lines = line_count(&request.replacement);
            Some(ApprovalPreview {
                summary: format!(
                    "编辑 {}：第 {}–{} 行 · {} 行 → {} 行（按参数估算）\n{}",
                    resolved.display(),
                    request.address.start_line,
                    request.address.end_line,
                    old_lines,
                    new_lines,
                    diff_excerpt(&prepared.located.text, &request.replacement)
                ),
                truncated: old_lines > MAX_DIFF_LINES || new_lines > MAX_DIFF_LINES,
                estimated: true,
                // 行区间规模由 `arguments` 直接合成，`preview` 只补文件系统事实（I72 §5.3）。
                entries: None,
                bytes: None,
                target_kind: Some("file".to_string()),
                exists: Some(true),
                symlink: None,
                verb_code: None,
                shell: None,
            })
        })
        .await
        .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match Request::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let Some(index_root) = context.index_root.clone() else {
            // 不静默降级为线性扫描：没有索引根就等于没有本工具的前提。
            return Ok(ToolResult::err(
                "index root is unavailable: super_edit only works when the application supplies \
                 one",
            ));
        };

        let probe_context = context.clone();
        let address = request.address;
        let probed = tokio::task::spawn_blocking(move || {
            let resolved = request.path.resolve_write(&probe_context)?;
            let metadata = match std::fs::metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(format!(
                        "'{}' does not exist; create it with the write tool first",
                        resolved.display()
                    ))
                }
                Err(error) => {
                    return Err(format!("cannot access '{}': {}", resolved.display(), error))
                }
            };
            if metadata.is_dir() {
                return Err(format!(
                    "'{}' is a directory; super_edit works on files only",
                    resolved.display()
                ));
            }
            if !metadata.is_file() {
                return Err(format!("'{}' is not a regular file", resolved.display()));
            }
            let revision = file_revision(&resolved)?;
            let prepared = prepare(&index_root, &resolved, metadata.len(), &revision, &address)?;
            Ok::<_, String>((resolved, revision, prepared))
        })
        .await;

        let (resolved, revision, prepared) = match probed {
            Ok(Ok(probed)) => probed,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("super_edit task failed: {}", error)),
        };

        let path_text = resolved.display().to_string();
        let located = &prepared.located;
        let empty_range = located.start == located.end;
        let replaced_chars = located.text.chars().count();

        // F7：非空区间没有 `expected` 等于「不校验就覆盖」，直接拒绝。
        if !empty_range && request.expected.is_none() {
            return Ok(ToolResult::err(format!(
                "'expected' is required: lines {}–{} of '{}' hold {} character(s), so this call \
                 would replace text you have not verified; pass the exact current text as \
                 'expected'",
                address.start_line, address.end_line, path_text, replaced_chars
            )));
        }

        // F8：`expected` 必须等于此刻磁盘上的区间文本（两侧都先归一 CRLF）。
        if let Some(expected) = request.expected.as_deref() {
            if normalize_newlines(expected) != normalize_newlines(&located.text) {
                return Ok(ToolResult::err(mismatch_message(
                    &path_text,
                    &address,
                    &located.text,
                )));
            }
        }

        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: path_text.clone(),
        };
        // 回执沿用本会话既有的行窗宽度；没有旧回执时用缺省值（字段必填，不能留空）。
        let line_window = self
            .receipts
            .lookup(&key)
            .await
            .map(|receipt| receipt.line_window_max_chars)
            .unwrap_or(DEFAULT_LINE_WINDOW_MAX_CHARS);

        let (start, end) = (located.start, located.end);
        let write_path = resolved.clone();
        let replacement = request.replacement.clone();
        let encoding = prepared.encoding;
        let has_bom = prepared.has_bom;
        let crlf = located.crlf;
        let written = tokio::task::spawn_blocking(move || {
            apply::apply_range(
                &write_path,
                apply::WriteRequest {
                    start,
                    end,
                    replacement: &replacement,
                    encoding,
                    has_bom,
                    crlf,
                    expected_revision: &revision,
                },
            )
        })
        .await;

        let write = match written {
            Ok(Ok(write)) => write,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("super_edit task failed: {}", error)),
        };

        // B→B 缓存卫生：先失效旧回执与旧索引，再登记本次回执（顺序不能反，
        // 否则刚写的回执会被同一路径前缀的失效规则一并清掉）。
        self.receipts.invalidate_path(&resolved).await;
        cache::invalidate_path(&resolved);
        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision: write.revision.clone(),
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: write.encoding.name().to_string(),
                    has_bom: write.has_bom,
                    truncated_lines: 0,
                    line_window_max_chars: line_window,
                },
            )
            .await;

        let new_chars = request.replacement.chars().count();
        let mut assistant_text = format!(
            "Edited {}: lines {}–{}, {} → {} character(s); the file is now {} bytes.\n{}",
            path_text,
            address.start_line,
            address.end_line,
            replaced_chars,
            new_chars,
            write.bytes_written,
            diff_excerpt(&located.text, &request.replacement)
        );
        assistant_text = append_footer(&assistant_text, &index_footer(&prepared));
        if let Some(line) = encoding_footer(prepared.encoding, prepared.has_bom, prepared.source) {
            assistant_text = append_footer(&assistant_text, &line);
        }

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "startLine": address.start_line,
                "endLine": address.end_line,
                "replacedChars": replaced_chars,
                "bytesWritten": write.bytes_written,
                "revision": write.revision,
            }),
            assistant_text,
        ))
    }
}

/// 解析后的入参：地址与文本都已是本工具内部形态，后续不再回读 JSON。
#[derive(Clone)]
struct Request {
    path: path::PathRef,
    address: Address,
    expected: Option<String>,
    replacement: String,
}

impl Request {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

        let start_line = required_u64(object, "startLine")?;
        if start_line == 0 {
            return Err("'startLine' must be >= 1".to_string());
        }
        let start_column = optional_u64(object, "startColumn")?.unwrap_or(1).max(1);

        let end_line = optional_u64(object, "endLine")?.unwrap_or(start_line);
        if end_line < start_line {
            // F9：倒序区间是参数错误，不进入定位。
            return Err(format!(
                "'endLine' {} must not be before 'startLine' {}",
                end_line, start_line
            ));
        }

        let end_column = optional_u64(object, "endColumn")?.map(|column| column.max(1));
        let to_end_of_line = optional_bool(object, "toEndOfLine")?.unwrap_or(false);
        if to_end_of_line && end_column.is_some() {
            // F9：两种「到行尾」写法互斥，同时给会让口径含糊。
            return Err(
                "'toEndOfLine' and 'endColumn' are mutually exclusive; pass only one of them"
                    .to_string(),
            );
        }

        let unit = match object.get("unit") {
            None | Some(Value::Null) => ColumnUnit::Char,
            Some(Value::String(label)) if label == "char" => ColumnUnit::Char,
            Some(Value::String(label)) if label == "byte" => ColumnUnit::Byte,
            Some(_) => return Err("'unit' must be \"char\" or \"byte\"".to_string()),
        };

        if unit == ColumnUnit::Char {
            // F6：字符口径列上限与 `super_read` 同一常量，超限请改用 byte 口径。
            for (label, column) in [
                ("startColumn", start_column),
                ("endColumn", end_column.unwrap_or(1)),
            ] {
                if column > MAX_COLUMN_CHARS {
                    return Err(format!(
                        "'{}' {} (unit = \"char\") exceeds the {} character cap; use unit = \
                         \"byte\" for deeper offsets",
                        label, column, MAX_COLUMN_CHARS
                    ));
                }
            }
        }

        let expected = optional_string(object, "expected")?;
        let replacement = optional_string(object, "replacement")?.unwrap_or_default();

        // `toEndOfLine` 只是一种更明确的「不写 endColumn」，归一后定位侧只见一种形态。
        let end_column = if to_end_of_line { None } else { end_column };

        Ok(Self {
            path,
            address: Address {
                start_line,
                start_column,
                end_line,
                end_column,
                unit,
            },
            expected,
            replacement,
        })
    }
}

/// 定位成功后的全部上下文：索引状态 + 编码 + 目标区间。
struct Prepared {
    file_bytes: u64,
    total_lines: u64,
    state: IndexState,
    build_ms: u64,
    encoding: &'static Encoding,
    has_bom: bool,
    source: EncodingSource,
    located: Located,
}

/// 建/取索引并完成定位，失败按 F6 / F10 口径原地重试一次。
///
/// 判据与 `super_read` 完全一致：都走 `super_index` 的 `detect` / `index_for` /
/// `rebuild_offsets` 与同一份 `proof` 边界表，区别只在「怎么定位行」——这里是地址，那边是窗口。
fn prepare(
    index_root: &Path,
    source_path: &Path,
    file_bytes: u64,
    revision: &str,
    address: &Address,
) -> Result<Prepared, String> {
    let (mut encoding, mut has_bom, _, mut source) = format::detect(source_path, None)?;
    let (mut offsets, mut state, mut build_ms) =
        cache::index_for(index_root, source_path, revision, file_bytes, None)?;
    let mut retried_after_utf8 = false;
    let mut retried_after_stale = false;

    loop {
        match locate::locate(source_path, &offsets, encoding, source, address) {
            Ok(located) => {
                return Ok(Prepared {
                    file_bytes,
                    total_lines: located.total_lines,
                    state,
                    build_ms,
                    encoding,
                    has_bom,
                    source,
                    located,
                })
            }
            // F10 之「按 UTF-8 建的索引与解码不符」：采样猜编码，UTF-16 需按新编码重建偏移。
            Err(LocateError::Index(IndexError::Utf8Fallback { .. })) if !retried_after_utf8 => {
                retried_after_utf8 = true;
                let sample = read_sample(source_path, GUESS_SAMPLE_BYTES)?;
                let (guessed, confident) = guess_encoding(&sample);
                if !confident {
                    return Err(format!(
                        "'{}': {}",
                        source_path.display(),
                        DecodeError::Undetectable
                    ));
                }
                if guessed == UTF_16LE || guessed == UTF_16BE {
                    let started = Instant::now();
                    offsets = cache::rebuild_offsets(index_root, source_path, revision, guessed)?;
                    state = IndexState::Built;
                    build_ms = started.elapsed().as_millis() as u64;
                }
                encoding = guessed;
                has_bom = false;
                source = EncodingSource::Guess;
            }
            // F10 之「revision 期间文件变了」：按原编码重建一次，仍失败才上报。
            Err(LocateError::Index(IndexError::StaleIndex { .. })) if !retried_after_stale => {
                retried_after_stale = true;
                let started = Instant::now();
                offsets = cache::rebuild_offsets(index_root, source_path, revision, encoding)?;
                state = IndexState::Built;
                build_ms = started.elapsed().as_millis() as u64;
            }
            Err(error) => return Err(error.message(source_path)),
        }
    }
}

fn read_sample(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot open '{}': {}", path.display(), error))?;
    let mut sample = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    while sample.len() < limit {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read '{}': {}", path.display(), error))?;
        if read == 0 {
            break;
        }
        sample.extend_from_slice(&buffer[..read]);
    }
    sample.truncate(limit);
    Ok(sample)
}

/// 索引 footer：与 `super_read` 逐字同形（§3.4 的 footer 是共享格式，不是各写一份）。
fn index_footer(prepared: &Prepared) -> String {
    match prepared.state {
        IndexState::Built => footer::render(
            footer::INDEX,
            format!(
                "built in {} ms · {} lines · {} bytes",
                prepared.build_ms, prepared.total_lines, prepared.file_bytes
            ),
        ),
        IndexState::Reused => footer::render(footer::INDEX, "reused"),
    }
}

/// 编码 footer：UTF-8 无 BOM 时静默，其余带来源后缀。
fn encoding_footer(
    encoding: &'static Encoding,
    has_bom: bool,
    source: EncodingSource,
) -> Option<String> {
    if source == EncodingSource::Utf8 && !has_bom {
        return None;
    }
    let label = encoding.name();
    Some(match source.suffix() {
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

/// F8 文案：把此刻磁盘上的区间文本按 `read` 的行号前缀形态回显。
///
/// 复用 `report::nearest_snippet` 的形态（带行号、可直接复制），而不是只给首 200 字：
/// `expected` 校验失败时模型需要的正是「该复制什么」，给片段比给长度更有用。
fn mismatch_message(path_text: &str, address: &Address, actual: &str) -> String {
    format!(
        "'expected' does not match the current text of lines {}–{} in '{}'; the file may have \
         changed since you last read it, or the text drifted. Whitespace counts, so copy the \
         text below exactly (the line-number column is not part of it):\n{}",
        address.start_line,
        address.end_line,
        path_text,
        numbered_excerpt(address.start_line, actual)
    )
}

/// 带行号的片段：行号列宽与分隔符与 `read` 同一常量，超宽行按字符截断。
fn numbered_excerpt(start_line: u64, text: &str) -> String {
    let normalized = normalize_newlines(text);
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    // 区间以行尾符结束时，末尾会多出一个空段：它不是「空行」，不参与回显。
    if normalized.ends_with('\n') {
        lines.pop();
    }
    if lines.is_empty() {
        return "(the range is empty)".to_string();
    }

    let mut rendered = String::new();
    for (index, line) in lines.iter().take(MAX_DIFF_LINES).enumerate() {
        rendered.push_str(&format!(
            "{:>width$}{}{}\n",
            start_line + index as u64,
            LINE_NUMBER_SEPARATOR,
            clip_chars(line, MISMATCH_EXCERPT_CHARS),
            width = LINE_NUMBER_WIDTH
        ));
    }
    if lines.len() > MAX_DIFF_LINES {
        rendered.push_str(&format!(
            "... ({} more line(s))\n",
            lines.len() - MAX_DIFF_LINES
        ));
    }
    rendered.trim_end_matches('\n').to_string()
}

/// 按字符截断：`expected` 不符时只需看出漂移方向，不把单行撑爆输出。
fn clip_chars(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index == limit {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

/// 归一 CRLF 后再比对：`expected` 里的换行风格不影响判定（I58 §3.3）。
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

fn diff_excerpt(old: &str, new: &str) -> String {
    let old = normalize_newlines(old);
    let new = normalize_newlines(new);
    let mut rendered = String::new();
    for line in old.lines().take(MAX_DIFF_LINES) {
        rendered.push_str("- ");
        rendered.push_str(line);
        rendered.push('\n');
    }
    let old_total = old.lines().count();
    if old_total > MAX_DIFF_LINES {
        rendered.push_str(&format!(
            "- ... ({} more lines)\n",
            old_total - MAX_DIFF_LINES
        ));
    }
    for line in new.lines().take(MAX_DIFF_LINES) {
        rendered.push_str("+ ");
        rendered.push_str(line);
        rendered.push('\n');
    }
    let new_total = new.lines().count();
    if new_total > MAX_DIFF_LINES {
        rendered.push_str(&format!(
            "+ ... ({} more lines)\n",
            new_total - MAX_DIFF_LINES
        ));
    }
    rendered.trim_end_matches('\n').to_string()
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("'{}' must be a string", key)),
    }
}
