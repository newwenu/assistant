use async_trait::async_trait;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcher;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind, SinkMatch};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::context::ToolUseContext;
use crate::exec::DEFAULT_TOOL_TIMEOUT;
use crate::os::path;
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
             adds surrounding lines. Respects `.gitignore`, skips hidden entries, and returns up \
             to 100 matching lines. Inline flags such as `(?i)` are supported."
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
                "context": { "type": "integer", "description": "Lines of context before and after each match (default 0)" }
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
        let search_path = base.clone();
        let search = tokio::task::spawn_blocking(move || {
            search(
                &search_path,
                &matcher,
                include.as_ref(),
                context_lines,
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
                "matches": outcome.lines
            }),
            assistant_text,
        ))
    }
}

fn assistant_text(outcome: &SearchOutcome, budget: Duration) -> String {
    if outcome.lines.is_empty() && !outcome.timed_out {
        return "(no matches)".to_string();
    }
    let mut text = outcome.lines.join("\n");
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
    text
}

struct GrepRequest {
    pattern: String,
    path: Option<String>,
    include: Option<String>,
    context: u64,
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

        Ok(Self {
            pattern,
            path,
            include,
            context,
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
    deadline: Instant,
) -> Result<SearchOutcome, String> {
    let mut outcome = SearchOutcome {
        lines: Vec::new(),
        match_count: 0,
        hit_limit: false,
        timed_out: false,
    };

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context as usize)
        .after_context(context as usize)
        .build();

    let walker = ignore::WalkBuilder::new(base)
        .hidden(true)
        .require_git(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .build();

    for entry in walker {
        if outcome.hit_limit || outcome.timed_out {
            break;
        }
        if Instant::now() >= deadline {
            outcome.timed_out = true;
            break;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let relative = match entry.path().strip_prefix(base) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if let Some(include) = include {
            if !include.is_match(&relative) {
                continue;
            }
        }

        let mut sink = FileSink {
            path: &relative,
            outcome: &mut outcome,
            deadline,
        };
        match searcher.search_path(matcher, entry.path(), &mut sink) {
            Ok(_) => {}
            Err(_) => continue,
        }
    }

    Ok(outcome)
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
            let text = trim_line_end(line);
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
        let text = trim_line_end(context.bytes());
        self.outcome
            .lines
            .push(format!("{}-{}-{}", self.path, number, text));
        Ok(true)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> Result<bool, std::io::Error> {
        if !self.outcome.lines.is_empty() {
            self.outcome.lines.push("--".to_string());
        }
        Ok(true)
    }
}

fn trim_line_end(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.trim_end_matches(['\n', '\r']).to_string()
}
