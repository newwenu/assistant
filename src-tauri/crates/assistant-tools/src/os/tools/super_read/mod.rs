//! `super_read`：按行索引读取任意行、任意列（I58 §3.2）。
//!
//! 与 `read` 的分工是**契约对齐、实现独立**（I58 §0.3）：行格式字面量、footer 顺序、
//! `data` 既有八键、回执六字段全部对齐；渲染（`render.rs`）自持，不消费 `agent_read`
//! 的 `LineSink` / `Scanner`。索引与行起点自证来自 B 私有公共层 `super_index`
//! （§7.2 / G18），与 `super_edit` 共用同一份判据。

use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::context::ToolUseContext;
use crate::os::agent_read::file_revision;
use crate::os::footer;
use crate::os::output::human_bytes;
use crate::os::path;
use crate::os::text_encoding::{
    guess_encoding, parse_optional_encoding, DecodeError, EncodingSource,
};
use crate::os::tools::super_index::{cache, format};
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::runtime::args::optional_u64;
use crate::storage::{
    ReadReceipt, ReadReceiptKey, ReadReceiptStore, DEFAULT_LINE_WINDOW_MAX_CHARS,
};

pub(crate) mod render;

use cache::IndexState;
use render::{ColumnUnit, RenderedWindow, WindowSpec};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "super_read";

/// `limit` 的取值上限：再大也只是把单次输出撑爆，分页仍由调用方决定。
const MAX_LIMIT: usize = 20_000;
/// `maxLineChars` 的取值下限 / 上限。
const MIN_MAX_LINE_CHARS: usize = render::MIN_LINE_WINDOW_CHARS;
const MAX_MAX_LINE_CHARS: usize = render::MAX_LINE_WINDOW_CHARS;
/// 猜编码时的采样上限：与 `read` 同量级，不为判定把大文件整个再读一遍。
const GUESS_SAMPLE_BYTES: usize = 1024 * 1024;

/// 路径失效：转调 `super_index::cache`，对外签名不变（I58 §4）。
///
/// 纯缓存卫生，**不承担正确性**：正确性由行起点自证兜底，所以 A 侧通用写工具
/// 不调用它；唯一调用方是 B 自己的 `super_edit`。
pub fn invalidate_path(path: &Path) {
    cache::invalidate_path(path);
}

/// 按行索引读取文本文件（I58 §3.2）。
///
/// 与 `read` 的差别只在「怎么定位行」：`read` 从头线性扫描，本工具按行起点表 `seek`。
/// 因此首次调用有建索引成本，之后同一 `revision` 直接复用落盘索引。
pub struct SuperReadTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl SuperReadTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for SuperReadTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Read a text file by line index: any line range, an in-line start column, and an \
             always-exact `totalLines`. Output lines are formatted exactly like `read`, so \
             results stay interchangeable. `unit` picks how `column` is measured: `char` \
             (default) counts decoded characters and caps at 200000; `byte` counts raw bytes of \
             the line content and must land on a character boundary, otherwise that line is \
             counted in `linesBeyondColumn`. The first call on a file pays a one-off indexing \
             cost."
                .to_string(),
        )
    }

    fn brief(&self) -> Option<&'static str> {
        Some(
            "Read by line index: any range, in-line column, exact `totalLines`. Request it when \
             `read` cannot express that.",
        )
    }

    fn manual(&self) -> Option<&'static str> {
        Some(
            "Cost: the first call on a file builds a line index (one full scan, capped at 64 MiB \
             of file); later calls seek by line offset and only read the requested window, so \
             they stay cheap on huge files. The index is cached per file revision and rebuilt \
             when the file changes.\n\n\
             When to use it: `read` already handles ordinary line-range paging — stay with it. \
             Reach for this tool only when `read` cannot express what you need: (1) you need an \
             exact `totalLines` and `read` reported `total unknown` because its scan hit the \
             budget; (2) you need to start inside a line (`column`), which `read` cannot do; \
             (3) a very long line was cut at the window width and you need the next segment of \
             it — pass the same line with a larger `column`.\n\n\
             Column units: `unit = \"char\"` counts decoded characters and is capped at 200000 \
             (CPU bound); `unit = \"byte\"` counts raw bytes of the line content and is unbounded, \
             but must land on a character boundary, otherwise the line is reported as \
             unreachable and counted in `linesBeyondColumn`.\n\n\
             Reading the same window with both tools yields byte-identical line text (footers \
             differ), so results stay interchangeable.",
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to read" },
                "offset": { "type": "integer", "description": "Start line (1-based; 0 is treated as 1)" },
                "limit": {
                    "type": "integer",
                    "description": "Max lines to read (1..=20000); the output cap of 2000 lines still applies"
                },
                "encoding": {
                    "type": "string",
                    "description": "Optional encoding label (e.g. gbk); omit to auto-detect, BOM wins"
                },
                "unit": {
                    "type": "string",
                    "enum": ["char", "byte"],
                    "description": "Unit of `column`: decoded characters (default) or raw bytes of the line content"
                },
                "column": {
                    "type": "integer",
                    "description": "In-line start position (1-based; 0 is treated as 1); `unit` decides the unit"
                },
                "maxLineChars": {
                    "type": "integer",
                    "description": "Visible width per line in characters (200..=20000, default 2000)"
                }
            },
            "required": ["path"]
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    /// 跟随 `read` 的勾选状态：用户不需要单独勾选它（I58 §3.6）。
    fn companion_of(&self) -> Option<&str> {
        Some("read")
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
        match Request::parse(input) {
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
        let request = Request::parse(input)?;
        let resolved = path::resolve_readable(&request.path, context)?;
        // 与 `read` 同构：工作区走 `read`，输出域走更严的独立动作。
        // `resource` 取命名空间**目录**，审批键覆盖本会话全部输出。
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
        let request = Request::parse(input).ok()?;
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            let resolved = path::resolve_readable(&request.path, &context).ok()?;
            // 工作区内读取由 `read` 的默认 `Allow` 覆盖，不走到审批卡片。
            if resolved.scope != path::ReadableScope::ToolOutput {
                return None;
            }
            let bytes = std::fs::metadata(&resolved.path).ok()?.len();
            let file_name = resolved.path.file_name()?.to_string_lossy().to_string();
            let first = request.offset;
            let last = first.saturating_add(render::line_cap(request.limit) as u64 - 1);
            let column = if request.spec.column > 1 {
                format!(
                    " · 列 {}（{}）",
                    request.spec.column,
                    request.spec.unit.label()
                )
            } else {
                String::new()
            };
            Some(ApprovalPreview {
                summary: format!(
                    "读本会话输出 {} · 第 {}–{} 行{} · {}",
                    file_name,
                    first,
                    last,
                    column,
                    human_bytes(bytes as usize)
                ),
                truncated: false,
                estimated: false,
                // 规模已由 `summary` 表达，不再重复产出结构化量级（I72 §5.3）。
                entries: None,
                bytes: None,
                target_kind: None,
                exists: None,
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
                "index root is unavailable: super_read only works when the application supplies \
                 one",
            ));
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
            prepare(
                &index_root,
                &resolved,
                metadata.len(),
                &revision,
                request.encoding,
                &request.spec,
            )
            .map(|prepared| (resolved, revision, prepared))
        })
        .await;

        let (resolved, revision, prepared) = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("super_read task failed: {}", error)),
        };

        let start_line = request.offset;
        let total = prepared.total_lines;
        let returned = prepared
            .window
            .as_ref()
            .map_or(0, |window| window.returned_lines);
        let end_line = (returned > 0).then(|| start_line + returned as u64 - 1);

        let mut assistant_text = if returned > 0 {
            prepared
                .window
                .as_ref()
                .map(|window| window.text.clone())
                .unwrap_or_default()
        } else if total == 0 {
            String::new()
        } else {
            // `read` 同文案；B 的总行数恒精确，因此不会出现「total unknown」态。
            format!(
                "No lines in range: file has {} lines, requested start line {}.",
                total, start_line
            )
        };

        if let Some(window) = prepared.window.as_ref().filter(|window| window.truncated) {
            assistant_text.push_str(&truncation_notice(window, total));
        }
        if let Some(footer) = long_lines_footer(&prepared, request.spec.max_line_chars) {
            assistant_text = append_footer(&assistant_text, &footer);
        }
        if let Some(footer) = column_footer(&request.spec) {
            assistant_text = append_footer(&assistant_text, &footer);
        }
        assistant_text = append_footer(&assistant_text, &prepared.index_footer());

        if let Some(footer) = encoding_footer(&prepared) {
            assistant_text = append_footer(&assistant_text, &footer);
        }

        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: resolved.display().to_string(),
        };
        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision,
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: prepared.encoding.name().to_string(),
                    has_bom: prepared.has_bom,
                    truncated_lines: prepared
                        .window
                        .as_ref()
                        .map_or(0, |window| window.truncated_lines),
                    // 与 `read` 同字段、同口径：B 写自己的宽度，`edit` 闸门文案自动正确。
                    line_window_max_chars: request.spec.max_line_chars,
                },
            )
            .await;

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": resolved.display().to_string(),
                "startLine": start_line,
                "endLine": end_line,
                "totalLines": total,
                "truncated": prepared.window.as_ref().is_some_and(|window| window.truncated),
                "truncatedLines": prepared.window.as_ref().map_or(0, |w| w.truncated_lines),
                "lineWindow": request.spec.max_line_chars,
                "linesBeyondColumn": prepared.window.as_ref().map_or(0, |w| w.lines_beyond_column),
                "index": {
                    "state": prepared.state.label(),
                    "buildMs": prepared.build_ms,
                    "lines": total,
                    "bytes": prepared.file_bytes,
                    "revision": prepared.revision
                }
            }),
            assistant_text,
        ))
    }
}

/// 一次调用的全部入参。
struct Request {
    path: String,
    offset: u64,
    limit: Option<usize>,
    encoding: Option<&'static Encoding>,
    spec: WindowSpec,
}

impl Request {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

        let offset = optional_u64(object, "offset")?.unwrap_or(1).max(1);
        let limit = match optional_u64(object, "limit")? {
            Some(0) => return Err("'limit' must be >= 1".to_string()),
            Some(value) if value > MAX_LIMIT as u64 => {
                return Err(format!("'limit' must be <= {}", MAX_LIMIT))
            }
            Some(value) => Some(value as usize),
            None => None,
        };
        let encoding = parse_optional_encoding(object)?;

        let unit = match object.get("unit") {
            None | Some(Value::Null) => ColumnUnit::Char,
            Some(Value::String(label)) if label == "char" => ColumnUnit::Char,
            Some(Value::String(label)) if label == "byte" => ColumnUnit::Byte,
            Some(_) => return Err("'unit' must be \"char\" or \"byte\"".to_string()),
        };

        let column = optional_u64(object, "column")?.unwrap_or(1).max(1);
        if unit == ColumnUnit::Char && column > render::MAX_COLUMN_CHARS {
            // F6：字符口径要逐字符推进，20 万是 CPU 上界；更深的列改用字节口径。
            return Err(format!(
                "'column' {} (unit = \"char\") exceeds the {} character cap; use unit = \"byte\" \
                 for deeper offsets",
                column,
                render::MAX_COLUMN_CHARS
            ));
        }

        let max_line_chars = match optional_u64(object, "maxLineChars")? {
            Some(value) if value < MIN_MAX_LINE_CHARS as u64 => {
                return Err(format!("'maxLineChars' must be >= {}", MIN_MAX_LINE_CHARS))
            }
            Some(value) if value > MAX_MAX_LINE_CHARS as u64 => {
                return Err(format!("'maxLineChars' must be <= {}", MAX_MAX_LINE_CHARS))
            }
            Some(value) => value as usize,
            None => DEFAULT_LINE_WINDOW_MAX_CHARS,
        };

        Ok(Self {
            path,
            offset,
            limit,
            encoding,
            spec: WindowSpec {
                offset,
                limit,
                column,
                unit,
                max_line_chars,
            },
        })
    }
}

/// 一次调用在阻塞线程里算出的全部事实。
struct Prepared {
    revision: String,
    total_lines: u64,
    file_bytes: u64,
    state: IndexState,
    build_ms: u64,
    window: Option<RenderedWindow>,
    encoding: &'static Encoding,
    has_bom: bool,
    source: EncodingSource,
}

impl Prepared {
    /// `[index]` footer：`built` 带耗时与规模，`reused` 只报告事实（I58 §3.4）。
    fn index_footer(&self) -> String {
        match self.state {
            IndexState::Built => footer::render(
                footer::INDEX,
                format!(
                    "built in {} ms · {} lines · {} bytes",
                    self.build_ms, self.total_lines, self.file_bytes
                ),
            ),
            IndexState::Reused => footer::render(footer::INDEX, "reused"),
        }
    }
}

/// 建 / 载索引 + 读窗口。索引是缓存：只有「源文件读不了」才是业务错误。
fn prepare(
    index_root: &Path,
    source_path: &Path,
    file_bytes: u64,
    revision: &str,
    explicit: Option<&'static Encoding>,
    spec: &WindowSpec,
) -> Result<Prepared, String> {
    // 第三项是内容起始偏移（BOM 已剥离）：索引起点表已按它起算，渲染无需再跳过 BOM。
    let (mut encoding, mut has_bom, _, mut source) = format::detect(source_path, explicit)?;

    let (mut offsets, mut state, mut build_ms) =
        cache::index_for(index_root, source_path, revision, file_bytes, explicit)?;

    let mut total_lines = offsets.len() as u64 - 1;
    let mut window = None;
    // 两种失败各只允许一次自愈，否则「重建 → 又失败」会变成死循环。
    let mut retried_after_utf8 = false;
    let mut retried_after_stale = false;

    // 起点表按单字节换行切；行数 ≥ 1 且起始行在范围内才需要读窗口。
    while total_lines > 0 && spec.offset <= total_lines {
        match render_window_slice(&offsets, total_lines, source_path, encoding, source, spec) {
            Ok(rendered) => {
                window = Some(rendered);
                break;
            }
            // UTF-8 乐观路径在窗口内遇到非法字节：与 `read` 同策略，整文件采样重猜一次。
            Err(render::RenderError::Utf8Fallback { .. }) if !retried_after_utf8 => {
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
                // UTF-16 的换行是双字节，单字节切出的行边界对它无效：按猜到的编码重建。
                if guessed == UTF_16LE || guessed == UTF_16BE {
                    let started = Instant::now();
                    offsets = cache::rebuild_offsets(index_root, source_path, revision, guessed)?;
                    total_lines = offsets.len() as u64 - 1;
                    state = IndexState::Built;
                    build_ms = started.elapsed().as_millis() as u64;
                }
                encoding = guessed;
                has_bom = false;
                source = EncodingSource::Guess;
            }
            // 行起点自证失败：索引与文件的真实换行位置不符，重建一次再试（I58 §4）。
            Err(render::RenderError::StaleIndex { .. }) if !retried_after_stale => {
                retried_after_stale = true;
                let started = Instant::now();
                offsets = cache::rebuild_offsets(index_root, source_path, revision, encoding)?;
                total_lines = offsets.len() as u64 - 1;
                state = IndexState::Built;
                build_ms = started.elapsed().as_millis() as u64;
            }
            Err(error) => return Err(error.message(source_path)),
        }
    }

    Ok(Prepared {
        revision: revision.to_string(),
        total_lines,
        file_bytes,
        state,
        build_ms,
        window,
        encoding,
        has_bom,
        source,
    })
}

/// 从整张起点表切出窗口再渲染：内存里只有窗口相关的 `seek` 与读缓冲。
fn render_window_slice(
    offsets: &[u64],
    total_lines: u64,
    source_path: &Path,
    encoding: &'static Encoding,
    source: EncodingSource,
    spec: &WindowSpec,
) -> Result<RenderedWindow, render::RenderError> {
    let io = |error: String| render::RenderError::Io {
        path: source_path.display().to_string(),
        error,
    };

    if spec.offset == 0 {
        return Err(io("'startLine' must be >= 1".to_string()));
    }
    if spec.offset > total_lines {
        return Err(io(format!(
            "line {} is past the end of the file ({} lines)",
            spec.offset, total_lines
        )));
    }

    let last_line = spec
        .offset
        .saturating_add(render::line_cap(spec.limit) as u64 - 1)
        .min(total_lines);
    let slice = &offsets[(spec.offset - 1) as usize..=last_line as usize];
    render::render_window(source_path, encoding, source, spec, slice, total_lines)
}

fn read_sample(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    let mut buffer = Vec::with_capacity(max_bytes);
    std::io::Read::take(file, max_bytes as u64)
        .read_to_end(&mut buffer)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    Ok(buffer)
}

/// 截断提示（`data.truncated` 为真时）：与 `read` 同一归因轴（窗口是否已满），
/// 但文案由 B 自写——B 的总行数恒精确，因此只有两态（I58 §0.4）。
fn truncation_notice(window: &RenderedWindow, total: u64) -> String {
    if window.window_full {
        footer::render(
            footer::TRUNCATED,
            format!(
                "showing {} of {} lines, {} bytes",
                window.returned_lines, total, window.rendered_bytes
            ),
        )
    } else {
        footer::render(
            footer::TRUNCATED,
            format!(
                "byte budget exhausted; showing {} of {} lines, {} bytes",
                window.returned_lines, total, window.rendered_bytes
            ),
        )
    }
}

/// 长行 footer（I57 §3.3）：只在本次输出确有被切的行时出现，宽度是本次的 `maxLineChars`。
fn long_lines_footer(prepared: &Prepared, width: usize) -> Option<String> {
    let cut = prepared
        .window
        .as_ref()
        .map_or(0, |window| window.truncated_lines);
    (cut > 0).then(|| {
        footer::render(
            footer::LONG_LINES,
            format!("{} lines cut at {} chars", cut, width),
        )
    })
}

/// 列 footer：只在 `column > 1` 时出现，说明可见段从哪一列开始（口径一并写出）。
fn column_footer(spec: &WindowSpec) -> Option<String> {
    (spec.column > 1).then(|| {
        footer::render(
            footer::COLUMN,
            format!("{} {}", spec.unit.label(), spec.column),
        )
    })
}

/// 编码 footer（上级 §14.4）：UTF-8 无 BOM 时静默，其余带来源后缀。
fn encoding_footer(prepared: &Prepared) -> Option<String> {
    if prepared.source == EncodingSource::Utf8 && !prepared.has_bom {
        return None;
    }
    let label = prepared.encoding.name();
    Some(match prepared.source.suffix() {
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
