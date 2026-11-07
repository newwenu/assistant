use async_trait::async_trait;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcher;
use grep_searcher::{
    BinaryDetection, Encoding as SearcherEncoding, Searcher, SearcherBuilder, Sink, SinkContext,
    SinkContextKind, SinkMatch,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::context::ToolUseContext;
use crate::exec::DEFAULT_TOOL_TIMEOUT;
use crate::os::path;
use crate::os::text_encoding::{parse_optional_encoding, BINARY_PROBE_BYTES};
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::ToolResult;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "grep";

/// 单次返回的匹配行数上限。
pub const MAX_MATCHES: usize = 100;
/// `context` 缺省值。
pub const DEFAULT_CONTEXT: u64 = 0;
/// 内部 deadline 相对外层超时预留的余量：留出把已收集结果回填的时间。
pub const DEADLINE_RESERVE: Duration = Duration::from_millis(500);
/// 零命中探测最多检查的文件数：只回答「scope 里有没有非 UTF-8 文件」，不做全量普查。
pub const PROBE_FILE_LIMIT: usize = 200;

pub struct AgentGrepTool {
    budget: Duration,
}

impl AgentGrepTool {
    pub fn new() -> Self {
        Self {
            budget: DEFAULT_TOOL_TIMEOUT,
        }
    }

    /// 内部时间上限可注入，仅用于测试「deadline 提前停止」。
    pub fn with_budget(budget: Duration) -> Self {
        Self { budget }
    }
}

impl Default for AgentGrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentGrepTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Search file contents with a regular expression. Output lines are `path:line:text`; \
             context lines use `path-line-text` and non-contiguous groups are separated by `--`. \
             `path` defaults to the workspace root, `include` filters files by glob, `context` \
             adds surrounding lines. Respects `.gitignore`, skips hidden and binary files, and \
             returns up to 100 matching lines. Inline flags such as `(?i)` are supported. Non-UTF-8 \
             files are matched as raw bytes unless `encoding` is passed, in which case the whole \
             search scope is transcoded first."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "Directory to search (defaults to the workspace root)" },
                "include": { "type": "string", "description": "File glob filter (e.g. '*.rs')" },
                "context": { "type": "integer", "description": "Lines of context before and after each match (default 0)" },
                "encoding": {
                    "type": "string",
                    "description": "Optional encoding for every file in the scope (e.g. gbk); omit to search raw UTF-8 bytes"
                }
            },
            "required": ["pattern"]
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
        let request = match GrepRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if let Err(error) = RegexMatcher::new(&request.pattern) {
            return ValidationResult::invalid(format!(
                "invalid regex '{}': {}",
                request.pattern, error
            ));
        }
        if let Err(error) = build_include(request.include.as_deref()) {
            return ValidationResult::invalid(error);
        }
        match base_directory(request.path.as_deref(), context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match GrepRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let matcher = match RegexMatcher::new(&request.pattern) {
            Ok(matcher) => matcher,
            Err(error) => {
                return Ok(ToolResult::err(format!(
                    "invalid regex '{}': {}",
                    request.pattern, error
                )))
            }
        };
        let include = match build_include(request.include.as_deref()) {
            Ok(include) => include,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let resolve_context = context.clone();
        let path_input = request.path.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let base = base_directory(path_input.as_deref(), &resolve_context)?;
            let metadata = std::fs::metadata(&base)
                .map_err(|error| format!("failed to stat '{}': {}", base.display(), error))?;
            if !metadata.is_dir() {
                return Err(format!("base path '{}' is not a directory", base.display()));
            }
            Ok::<PathBuf, String>(base)
        })
        .await;

        let base = match prepared {
            Ok(Ok(base)) => base,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("grep task failed: {}", error)),
        };

        let budget = self
            .budget
            .checked_sub(DEADLINE_RESERVE)
            .unwrap_or(self.budget);
        let deadline = Instant::now() + budget;
        let context_lines = request.context;
        let encoding = request.encoding.clone();
        let search_path = base.clone();
        let search = tokio::task::spawn_blocking(move || {
            search(
                &search_path,
                &matcher,
                include.as_ref(),
                context_lines,
                encoding,
                deadline,
            )
        })
        .await;

        let outcome = match search {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("grep task failed: {}", error)),
        };

        let assistant_text = assistant_text(&outcome, self.budget);

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": base.display().to_string(),
                "pattern": request.pattern,
                "include": request.include,
                "context": request.context,
                "totalMatches": outcome.match_count,
                "truncated": outcome.hit_limit,
                "timedOut": outcome.timed_out,
                "binarySkipped": outcome.binary_skipped,
                "lossyLines": outcome.lossy_lines,
                "nonUtf8Probe": outcome.probe.as_ref().map(|probe| serde_json::json!({
                    "files": probe.files,
                    "truncated": probe.truncated
                })),
                "matches": outcome.lines
            }),
            assistant_text,
        ))
    }
}

fn assistant_text(outcome: &SearchOutcome, budget: Duration) -> String {
    let mut text = if outcome.lines.is_empty() && !outcome.timed_out {
        "(no matches)".to_string()
    } else {
        outcome.lines.join("\n")
    };
    if outcome.timed_out {
        text.push_str(&format!(
            "\n[search stopped at the {}ms time budget; results may be incomplete; narrow 'path' \
             or 'include']",
            budget.as_millis()
        ));
    } else if outcome.hit_limit {
        text.push_str(&format!(
            "\n[truncated: showing the first {} matching lines; narrow 'path' or 'include']",
            MAX_MATCHES
        ));
    }
    // footer 形态固定为 `[non-utf8: N file(s)]`（[I38c-grep §3.1]）；
    // 是否被截断只进 `nonUtf8Probe.truncated`，不延长 footer。
    if let Some(probe) = &outcome.probe {
        if probe.files > 0 {
            text.push_str(&format!("\n[non-utf8: {} file(s)]", probe.files));
        }
    }
    text
}

struct GrepRequest {
    pattern: String,
    path: Option<String>,
    include: Option<String>,
    context: u64,
    encoding: Option<SearcherEncoding>,
}

impl GrepRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let pattern = object
            .get("pattern")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing or empty 'pattern' field".to_string())?
            .to_string();

        let path = match object.get("path") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "'path' must be a non-empty string".to_string())?
                    .to_string(),
            ),
        };

        let include = match object.get("include") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "'include' must be a non-empty string".to_string())?
                    .to_string(),
            ),
        };

        let context = match object.get("context") {
            None | Some(Value::Null) => DEFAULT_CONTEXT,
            Some(value) => value
                .as_u64()
                .ok_or_else(|| "'context' must be a non-negative integer".to_string())?,
        };

        // label 校验与 `read` / `write` 共用一套（别名表与错误文案不分叉）；
        // searcher 只接受自己的 `Encoding` 类型，用规范化名重建即可。
        let encoding = match parse_optional_encoding(object)? {
            None => None,
            Some(resolved) => Some(SearcherEncoding::new(resolved.name()).map_err(|error| {
                format!(
                    "cannot search with encoding '{}': {}",
                    resolved.name(),
                    error
                )
            })?),
        };

        Ok(Self {
            pattern,
            path,
            include,
            context,
            encoding,
        })
    }
}

fn build_include(pattern: Option<&str>) -> Result<Option<GlobSet>, String> {
    match pattern {
        None => Ok(None),
        Some(pattern) => {
            let glob = GlobBuilder::new(pattern)
                .literal_separator(false)
                .backslash_escape(false)
                .build()
                .map_err(|error| format!("invalid include glob '{}': {}", pattern, error))?;
            let mut builder = GlobSetBuilder::new();
            builder.add(glob);
            builder
                .build()
                .map(Some)
                .map_err(|error| format!("invalid include glob '{}': {}", pattern, error))
        }
    }
}

fn base_directory(path_input: Option<&str>, context: &ToolUseContext) -> Result<PathBuf, String> {
    match path_input {
        Some(input) => path::resolve(input, context),
        None => {
            let root = context.workspace_root.as_deref().ok_or_else(|| {
                "'path' is required when the context has no workspace_root; pass an explicit \
                 directory"
                    .to_string()
            })?;
            path::resolve(root, context)
        }
    }
}

struct SearchOutcome {
    lines: Vec<String>,
    match_count: usize,
    hit_limit: bool,
    timed_out: bool,
    binary_skipped: usize,
    lossy_lines: usize,
    probe: Option<NonUtf8Probe>,
}

/// 零命中探测结论（上级 §14.7）：只回答「scope 里有没有非 UTF-8 文件」。
struct NonUtf8Probe {
    files: usize,
    truncated: bool,
}

/// 同步搜索，必须在 `spawn_blocking` 内调用。
///
/// 内部 deadline 早于外层超时：`spawn_blocking` 的线程无法被外层取消，
/// 到点必须自己停下并返回已收集结果，否则外层只能放弃 future 而线程继续空转。
fn search(
    base: &Path,
    matcher: &RegexMatcher,
    include: Option<&GlobSet>,
    context: u64,
    encoding: Option<SearcherEncoding>,
    deadline: Instant,
) -> Result<SearchOutcome, String> {
    let mut outcome = SearchOutcome {
        lines: Vec::new(),
        match_count: 0,
        hit_limit: false,
        timed_out: false,
        binary_skipped: 0,
        lossy_lines: 0,
        probe: None,
    };

    // `binary_detection` 默认是 `None`（等于不做检测），必须显式开启：
    // 否则二进制文件会被当成文本，产出乱码命中行（上级 §14.7）。
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context as usize)
        .after_context(context as usize)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .encoding(encoding)
        .build();

    for entry in walk(base) {
        if outcome.hit_limit || outcome.timed_out {
            break;
        }
        if Instant::now() >= deadline {
            outcome.timed_out = true;
            break;
        }

        let Ok(entry) = entry else {
            continue;
        };
        let Some(path) = searchable_path(&entry, base, include) else {
            continue;
        };

        let mut sink = FileSink {
            path: &path,
            outcome: &mut outcome,
            deadline,
        };
        match searcher.search_path(matcher, entry.path(), &mut sink) {
            Ok(_) => {}
            Err(_) => continue,
        }
    }

    // 冷路径：只在零命中且未超时时执行，热路径零成本。
    if outcome.match_count == 0 && !outcome.timed_out {
        outcome.probe = Some(probe_non_utf8(base, include, deadline));
    }

    Ok(outcome)
}

fn walk(base: &Path) -> ignore::Walk {
    ignore::WalkBuilder::new(base)
        .hidden(true)
        .require_git(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .build()
}

/// 统一的「这个条目要不要搜」判定：普通文件 + 相对路径 + `include` 过滤。
fn searchable_path(
    entry: &ignore::DirEntry,
    base: &Path,
    include: Option<&GlobSet>,
) -> Option<String> {
    if !entry
        .file_type()
        .map(|kind| kind.is_file())
        .unwrap_or(false)
    {
        return None;
    }
    let relative = entry.path().strip_prefix(base).ok()?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    match include {
        Some(include) if !include.is_match(&relative) => None,
        _ => Some(relative),
    }
}

/// 逐文件读前 8KB，统计「非 UTF-8 且非二进制」文件数（上级 §14.7 冷路径）。
///
/// 复用单个缓冲；有 BOM 的文件已可搜（`bom_sniffing`），合法 UTF-8 的文件本来就搜过了，
/// 两者都不计入。达到 [`PROBE_FILE_LIMIT`] 或 deadline 即停并标记 `truncated`。
fn probe_non_utf8(base: &Path, include: Option<&GlobSet>, deadline: Instant) -> NonUtf8Probe {
    let mut probe = NonUtf8Probe {
        files: 0,
        truncated: false,
    };
    let mut buffer = Vec::with_capacity(BINARY_PROBE_BYTES);
    let mut checked = 0usize;

    for entry in walk(base) {
        let Ok(entry) = entry else {
            continue;
        };
        if searchable_path(&entry, base, include).is_none() {
            continue;
        }
        if checked >= PROBE_FILE_LIMIT {
            probe.truncated = true;
            break;
        }
        if Instant::now() >= deadline {
            probe.truncated = true;
            break;
        }
        checked += 1;

        buffer.clear();
        let read = read_prefix(entry.path(), &mut buffer);
        if read == PrefixKind::NonUtf8 {
            probe.files += 1;
        }
    }

    probe
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrefixKind {
    /// 合法 UTF-8，或只是被 8KB 窗口截断（尾部半个字符）。
    Utf8,
    /// 有 BOM：`bom_sniffing` 已能搜。
    Bom,
    /// 含 `0x00`：二进制。
    Binary,
    /// 既非 UTF-8 也非二进制：`read` 会要求显式 `encoding`。
    NonUtf8,
}

fn read_prefix(path: &Path, buffer: &mut Vec<u8>) -> PrefixKind {
    use std::io::Read as _;

    let Ok(file) = std::fs::File::open(path) else {
        return PrefixKind::Utf8;
    };
    match std::io::BufReader::new(file)
        .take(BINARY_PROBE_BYTES as u64)
        .read_to_end(buffer)
    {
        Ok(_) => classify_prefix(buffer),
        Err(_) => PrefixKind::Utf8,
    }
}

fn classify_prefix(prefix: &[u8]) -> PrefixKind {
    if encoding_rs::Encoding::for_bom(prefix).is_some() {
        return PrefixKind::Bom;
    }
    if prefix.contains(&0) {
        return PrefixKind::Binary;
    }
    match std::str::from_utf8(prefix) {
        Ok(_) => PrefixKind::Utf8,
        // `error_len == None` 是「读到一半」而非「非法字节」，不能算非 UTF-8。
        Err(error) if error.error_len().is_none() => PrefixKind::Utf8,
        Err(_) => PrefixKind::NonUtf8,
    }
}

struct FileSink<'a> {
    path: &'a str,
    outcome: &'a mut SearchOutcome,
    deadline: Instant,
}

impl Sink for FileSink<'_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        if self.outcome.match_count >= MAX_MATCHES {
            self.outcome.hit_limit = true;
            return Ok(false);
        }
        if Instant::now() >= self.deadline {
            self.outcome.timed_out = true;
            return Ok(false);
        }
        let first_line = mat.line_number().unwrap_or(0);
        for (index, line) in mat.lines().enumerate() {
            let number = first_line + index as u64;
            let (text, lossy) = trim_line_end(line);
            if lossy {
                self.outcome.lossy_lines += 1;
            }
            self.outcome
                .lines
                .push(format!("{}:{}:{}", self.path, number, text));
        }
        self.outcome.match_count += 1;
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> Result<bool, std::io::Error> {
        if matches!(context.kind(), SinkContextKind::Other) {
            return Ok(true);
        }
        let number = context.line_number().unwrap_or(0);
        // `lossyLines` 只统计命中行（上级 §14.7），上下文行不参与计数。
        let (text, _) = trim_line_end(context.bytes());
        self.outcome
            .lines
            .push(format!("{}-{}-{}", self.path, number, text));
        Ok(true)
    }

    fn binary_data(
        &mut self,
        _searcher: &Searcher,
        _binary_byte_offset: u64,
    ) -> Result<bool, std::io::Error> {
        self.outcome.binary_skipped += 1;
        // 返回 false 即放弃本文件：`quit` 语义下继续读没有意义，只会产出乱码行。
        Ok(false)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> Result<bool, std::io::Error> {
        if !self.outcome.lines.is_empty() {
            self.outcome.lines.push("--".to_string());
        }
        Ok(true)
    }
}

/// 去掉行尾换行；返回文本是否含替换字符（原始字节非法 UTF-8）。
///
/// 先走 `from_utf8` 快速路径（合法时零分配），失败才 `from_utf8_lossy`
/// 并置 `lossy`，让调用方能在结果里如实报告「这一行不是 UTF-8」。
fn trim_line_end(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.trim_end_matches(['\n', '\r']).to_string(), false),
        Err(_) => (
            String::from_utf8_lossy(bytes)
                .trim_end_matches(['\n', '\r'])
                .to_string(),
            true,
        ),
    }
}
