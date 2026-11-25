mod outcome;
mod probe;
mod render;
mod request;
mod search;
mod sink;

use async_trait::async_trait;
use grep_regex::RegexMatcher;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::context::ToolUseContext;
use crate::exec::DEFAULT_TOOL_TIMEOUT;
use crate::os::scope;
use crate::r#trait::{
    BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier, ValidationResult,
};
use crate::result::ToolResult;

use render::assistant_text;
use request::{build_include, GrepRequest};
use search::search;

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
        match scope::base_directory(request.path.as_ref(), context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = GrepRequest::parse(input)?;
        let base = scope::base_directory(request.path.as_ref(), context)?;
        Ok(vec![PermissionIntent::new(
            "read",
            base.display().to_string(),
        )])
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
        let prepared = tokio::task::spawn_blocking(move || {
            let base = scope::base_directory(request.path.as_ref(), &resolve_context)?;
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
