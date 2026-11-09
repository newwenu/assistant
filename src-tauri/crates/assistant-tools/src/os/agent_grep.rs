use async_trait::async_trait;
use globset::GlobSet;
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
use crate::os::agent_glob;
use crate::os::path;
use crate::os::text_encoding::{parse_optional_encoding, BINARY_PROBE_BYTES};
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::ToolResult;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "grep";

/// 单次返回的命中块上限。
pub const MAX_MATCHES: usize = 100;
/// 单次返回的渲染行上限：命中行与上下文行合计。
///
/// `MAX_MATCHES` 只数命中块，上下文行完全绕过它——大 `context` 配一个靠前的命中
/// 就能把整个文件塞进结果，因此必须另有行数闸门（[grep §3.2]）。
pub const MAX_OUTPUT_LINES: usize = 300;
/// `context` 上限。超出直接拒绝而非静默夹紧：夹紧会让模型误以为上下文真的给了那么多。
pub const MAX_CONTEXT: u64 = 10;
/// `context` 缺省值。
pub const DEFAULT_CONTEXT: u64 = 0;
/// 单文件体积上限：单文件在 `spawn_blocking` 内无法被 deadline 中断，
/// 体积是唯一能在「一个文件」粒度收口的杠杆（[grep §3.3]）。
pub const MAX_FILESIZE: u64 = 16 * 1024 * 1024;
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
             caps output at 100 matches / 300 lines. Inline flags such as `(?i)` are supported. \
             Non-UTF-8 files are matched as raw bytes unless `encoding` is passed, in which case \
             the whole search scope is transcoded first."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern (single-line match, e.g. 'fn main')"
                },
                "path": { "type": "string", "description": "Directory to search" },
                "include": { "type": "string", "description": "File glob filter (e.g. '*.rs')" },
                "context": {
                    "type": "integer",
                    "description": "Lines of context before and after each match (0-10, default 0)"
                },
                "encoding": {
                    "type": "string",
                    "description": "Optional encoding label (e.g. gbk) for the whole search; omit to search as-is"
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

        let mut outcome = match search {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("grep task failed: {}", error)),
        };

        let lines = outcome.take_lines();
        let assistant_text = assistant_text(&outcome, &lines, self.budget);

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": base.display().to_string(),
                "pattern": request.pattern,
                "include": request.include,
                "context": request.context,
                "totalMatches": outcome.match_count,
                "truncated": outcome.truncated(),
                "timedOut": outcome.timed_out,
                "binarySkipped": outcome.binary_skipped,
                "largeSkipped": outcome.large_skipped,
                "unreadableFiles": outcome.unreadable,
                "lossyLines": outcome.lossy_lines,
                "nonUtf8Probe": outcome.probe.as_ref().map(|probe| serde_json::json!({
                    "files": probe.files,
                    "truncated": probe.truncated
                })),
                "matches": lines
            }),
            assistant_text,
        ))
    }
}

fn assistant_text(outcome: &SearchOutcome, lines: &[String], budget: Duration) -> String {
    // 超时且零行时不写 `(no matches)`，否则会多出一个前导空行（footer 前只留提示行）。
    let mut text = if lines.is_empty() {
        if outcome.timed_out {
            String::new()
        } else {
            "(no matches)".to_string()
        }
    } else {
        lines.join("\n")
    };

    if outcome.timed_out {
        push_line(
            &mut text,
            &format!(
                "[search stopped at the {}ms time budget; results may be incomplete; narrow \
                 'path' or 'include']",
                budget.as_millis()
            ),
        );
    } else if outcome.match_limit {
        push_line(
            &mut text,
            &format!(
                "[truncated: showing the first {} matching lines; narrow 'path' or 'include']",
                MAX_MATCHES
            ),
        );
    } else if outcome.line_limit {
        push_line(
            &mut text,
            &format!(
                "[truncated: output line limit ({}) reached; narrow 'path', 'include', or reduce \
                 'context']",
                MAX_OUTPUT_LINES
            ),
        );
    }

    // footer 标记集：各自独占一行、只给事实（[I38c §14.4] / [grep §3.1]）。
    if let Some(probe) = &outcome.probe {
        if probe.files > 0 {
            push_line(&mut text, &format!("[non-utf8: {} file(s)]", probe.files));
        }
    }
    let skipped = outcome.skipped_files();
    if skipped > 0 {
        push_line(&mut text, &format!("[skipped: {} file(s)]", skipped));
    }
    if outcome.lossy_lines > 0 {
        push_line(
            &mut text,
            &format!("[lossy: {} line(s)]", outcome.lossy_lines),
        );
    }
    text
}

fn push_line(text: &mut String, line: &str) {
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
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

        // `pattern` 原样编译，不做 `trim`：前导 / 尾随空格是正则的一部分（搜缩进、搜行尾空格）。
        // `trim` 只用于拒绝「全空白」输入，不改变编译用的字符串（[grep §3]）。
        let pattern = object
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'pattern' field".to_string())?;
        if pattern.trim().is_empty() {
            return Err("'pattern' must not be blank".to_string());
        }
        let pattern = pattern.to_string();

        let path = path::parse_optional_path_arg(object, "path")?;

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
            Some(value) => {
                let value = value
                    .as_u64()
                    .ok_or_else(|| "'context' must be a non-negative integer".to_string())?;
                if value > MAX_CONTEXT {
                    return Err(format!(
                        "'context' must be between 0 and {} (got {})",
                        MAX_CONTEXT, value
                    ));
                }
                value
            }
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
        Some(pattern) => agent_glob::build_glob_set(&[pattern.to_string()])
            .map(Some)
            .map_err(|error| format!("invalid include glob '{}': {}", pattern, error)),
    }
}

fn base_directory(path_input: Option<&str>, context: &ToolUseContext) -> Result<PathBuf, String> {
    match path_input {
        Some(input) => path::resolve(input, context),
        None => {
            let root = path::resolution_base(context).ok_or_else(|| {
                "'path' is required when the context has no workspace_root or resolve_base; pass \
                 an explicit directory"
                    .to_string()
            })?;
            path::resolve(root, context)
        }
    }
}

struct SearchOutcome {
    /// 已提交的文件（相对路径 + 该文件的渲染行）。文件内行号升序，文件间最后按路径排序。
    files: Vec<(String, Vec<String>)>,
    /// 已提交的命中块数。
    match_count: usize,
    /// 已提交的渲染行数（含上下文行与 `--` 分隔符）。
    line_count: usize,
    match_limit: bool,
    line_limit: bool,
    timed_out: bool,
    binary_skipped: usize,
    large_skipped: usize,
    unreadable: usize,
    lossy_lines: usize,
    probe: Option<NonUtf8Probe>,
}

impl SearchOutcome {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            match_count: 0,
            line_count: 0,
            match_limit: false,
            line_limit: false,
            timed_out: false,
            binary_skipped: 0,
            large_skipped: 0,
            unreadable: 0,
            lossy_lines: 0,
            probe: None,
        }
    }

    fn stopped(&self) -> bool {
        self.match_limit || self.line_limit || self.timed_out
    }

    /// 任一输出上限触发（`data.truncated` 的语义）。
    fn truncated(&self) -> bool {
        self.match_limit || self.line_limit
    }

    fn skipped_files(&self) -> usize {
        self.binary_skipped + self.large_skipped + self.unreadable
    }

    /// 文件间按路径字典序提交，保证同一子集的输出顺序可复现（[grep §3.3]）。
    fn take_lines(&mut self) -> Vec<String> {
        self.files.sort_by(|left, right| left.0.cmp(&right.0));
        let mut lines = Vec::with_capacity(self.line_count);
        for (_, file_lines) in self.files.drain(..) {
            lines.extend(file_lines);
        }
        lines
    }
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
    let mut outcome = SearchOutcome::new();
    // 显式指定 `encoding` 时整个 scope 已按该编码转码，非 UTF-8 文件也被搜到了，
    // 探测结论没有信息量，跳过（[grep §3.4]）。
    let explicit_encoding = encoding.is_some();

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
        if outcome.stopped() {
            break;
        }
        if Instant::now() >= deadline {
            outcome.timed_out = true;
            break;
        }

        let Ok(entry) = entry else {
            continue;
        };
        let relative = match decide_file(&entry, base, include) {
            FileDecision::Search(relative) => relative,
            FileDecision::Large => {
                outcome.large_skipped += 1;
                continue;
            }
            FileDecision::Skip => continue,
        };

        let mut sink = FileSink::new(relative, deadline, &mut outcome);
        let search_result = searcher.search_path(matcher, entry.path(), &mut sink);
        let failed = search_result.is_err();
        sink.finish(failed);
        drop(sink);
        if failed {
            outcome.unreadable += 1;
        }
    }

    // 冷路径：只在零命中、未超时且未显式指定 `encoding` 时执行，热路径零成本。
    if outcome.match_count == 0 && !outcome.timed_out && !explicit_encoding {
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

enum FileDecision {
    /// 参与搜索，携带相对 `base` 的 `/` 分隔路径（`include` 与输出都用它）。
    Search(String),
    /// 超过单文件体积上限：不搜、不探测，计入 `largeSkipped`。
    Large,
    /// 目录、符号链接、`include` 不匹配。
    Skip,
}

/// 统一的「这个条目要不要搜」判定：普通文件 + 体积上限 + `include` 过滤。
fn decide_file(entry: &ignore::DirEntry, base: &Path, include: Option<&GlobSet>) -> FileDecision {
    if !entry
        .file_type()
        .map(|kind| kind.is_file())
        .unwrap_or(false)
    {
        return FileDecision::Skip;
    }
    if entry.metadata().map(|meta| meta.len()).unwrap_or(0) > MAX_FILESIZE {
        return FileDecision::Large;
    }
    let relative = match entry.path().strip_prefix(base) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => return FileDecision::Skip,
    };
    match include {
        Some(include) if !include.is_match(&relative) => FileDecision::Skip,
        _ => FileDecision::Search(relative),
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
        if !matches!(decide_file(&entry, base, include), FileDecision::Search(_)) {
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

/// 单文件缓冲：命中行与上下文行先落到这里，文件结束时整体提交。
///
/// 按文件缓冲是「二进制整文件丢弃」与「文件间排序」的前提——逐行即时输出既无法
/// 在发现 NUL 后撤回已产出的行，也无法把同一文件的行聚在一起排序。
struct FileSink<'a> {
    path: String,
    outcome: &'a mut SearchOutcome,
    deadline: Instant,
    buffer: Vec<String>,
    matches: usize,
    lossy: usize,
    binary: bool,
}

impl<'a> FileSink<'a> {
    fn new(path: String, deadline: Instant, outcome: &'a mut SearchOutcome) -> Self {
        Self {
            path,
            outcome,
            deadline,
            buffer: Vec::new(),
            matches: 0,
            lossy: 0,
            binary: false,
        }
    }

    /// 当前文件还能再放多少行（命中行 + 上下文行 + 分隔符共用同一预算）。
    fn lines_left(&self) -> usize {
        MAX_OUTPUT_LINES.saturating_sub(self.outcome.line_count + self.buffer.len())
    }

    /// 提交或丢弃本文件的缓冲。
    fn finish(&mut self, failed: bool) {
        if failed {
            return;
        }
        if self.binary {
            // 二进制文件整文件丢弃：NUL 之前也可能已有命中，逐行输出会把乱码混进结果。
            self.outcome.binary_skipped += 1;
            return;
        }
        self.outcome.match_count += self.matches;
        self.outcome.lossy_lines += self.lossy;
        self.outcome.line_count += self.buffer.len();
        if !self.buffer.is_empty() {
            let lines = std::mem::take(&mut self.buffer);
            self.outcome.files.push((self.path.clone(), lines));
        }
    }
}

impl Sink for FileSink<'_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        if self.outcome.match_count + self.matches >= MAX_MATCHES {
            self.outcome.match_limit = true;
            return Ok(false);
        }
        if Instant::now() >= self.deadline {
            self.outcome.timed_out = true;
            return Ok(false);
        }

        let lines: Vec<&[u8]> = mat.lines().collect();
        if lines.len() > self.lines_left() {
            self.outcome.line_limit = true;
            return Ok(false);
        }

        let first_line = mat.line_number().unwrap_or(0);
        for (index, line) in lines.into_iter().enumerate() {
            let number = first_line + index as u64;
            let (text, lossy) = trim_line_end(line);
            if lossy {
                self.lossy += 1;
            }
            self.buffer
                .push(format!("{}:{}:{}", self.path, number, text));
        }
        self.matches += 1;
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
        // 上下文行不经过 `matched`，大 `context` 的海量行只在这里出现，必须自查 deadline。
        if Instant::now() >= self.deadline {
            self.outcome.timed_out = true;
            return Ok(false);
        }
        if self.lines_left() == 0 {
            self.outcome.line_limit = true;
            return Ok(false);
        }

        let number = context.line_number().unwrap_or(0);
        // 上下文行同样以替换字符渲染给模型，因此与命中行一起计入 `lossyLines`。
        let (text, lossy) = trim_line_end(context.bytes());
        if lossy {
            self.lossy += 1;
        }
        self.buffer
            .push(format!("{}-{}-{}", self.path, number, text));
        Ok(true)
    }

    fn binary_data(
        &mut self,
        _searcher: &Searcher,
        _binary_byte_offset: u64,
    ) -> Result<bool, std::io::Error> {
        self.binary = true;
        // 返回 false 即放弃本文件：`quit` 语义下继续读没有意义，只会产出乱码行。
        Ok(false)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> Result<bool, std::io::Error> {
        if self.buffer.is_empty() {
            return Ok(true);
        }
        if self.lines_left() == 0 {
            self.outcome.line_limit = true;
            return Ok(false);
        }
        self.buffer.push("--".to_string());
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
