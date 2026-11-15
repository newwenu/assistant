use async_trait::async_trait;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;
use crate::os::path;
use crate::r#trait::{
    BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier, ValidationResult,
};
use crate::result::ToolResult;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "glob";

/// 单次返回的匹配条目上限。
pub const MAX_MATCHES: usize = 1000;

pub struct AgentGlobTool;

impl AgentGlobTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentGlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentGlobTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Find files by glob pattern, relative to `path` (defaults to the workspace root). \
             Patterns use shell semantics: `*` and `?` do not cross directory separators, `**` \
             does. `exclude` patterns are applied after matching. Respects `.gitignore` and skips \
             hidden entries. Returns up to 1000 matches sorted by path."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern relative to 'path' (e.g. '**/*.ts')" },
                "path": { "type": "string", "description": "Base directory (defaults to the workspace root)" },
                "exclude": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Glob patterns to exclude from the matches"
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
        let request = match GlobRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if let Err(error) = build_matcher(&request.pattern, &request.exclude) {
            return ValidationResult::invalid(error);
        }
        match base_directory(request.path.as_deref(), context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = GlobRequest::parse(input)?;
        let base = base_directory(request.path.as_deref(), context)?;
        Ok(vec![PermissionIntent::new(
            "read",
            base.display().to_string(),
        )])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match GlobRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        let matcher = match build_matcher(&request.pattern, &request.exclude) {
            Ok(matcher) => matcher,
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
            Err(error) => return Err(format!("glob task failed: {}", error)),
        };

        let walk_path = base.clone();
        let walk = tokio::task::spawn_blocking(move || collect_matches(&walk_path, &matcher)).await;

        let collected = match walk {
            Ok(Ok(collected)) => collected,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("glob task failed: {}", error)),
        };

        let total = collected.total;
        let truncated = total > MAX_MATCHES;
        let returned: Vec<String> = collected.matches.into_iter().take(MAX_MATCHES).collect();

        let assistant_text = if returned.is_empty() {
            "(no matches)".to_string()
        } else if truncated {
            format!(
                "{}\n[truncated: showing {} of {} items; narrow the pattern or pass a more \
                 specific 'path']",
                returned.join("\n"),
                returned.len(),
                total
            )
        } else {
            returned.join("\n")
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": base.display().to_string(),
                "pattern": request.pattern,
                "exclude": request.exclude,
                "totalMatches": total,
                "truncated": truncated,
                "matches": returned
            }),
            assistant_text,
        ))
    }
}

struct GlobRequest {
    pattern: String,
    path: Option<String>,
    exclude: Vec<String>,
}

impl GlobRequest {
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

        let path = path::parse_optional_path_arg(object, "path")?;

        let exclude = match object.get("exclude") {
            None | Some(Value::Null) => Vec::new(),
            Some(value) => {
                let array = value
                    .as_array()
                    .ok_or_else(|| "'exclude' must be an array of strings".to_string())?;
                let mut patterns = Vec::with_capacity(array.len());
                for item in array {
                    let text = item
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| "'exclude' entries must be non-empty strings".to_string())?;
                    patterns.push(text.to_string());
                }
                patterns
            }
        };

        Ok(Self {
            pattern,
            path,
            exclude,
        })
    }
}

struct Matcher {
    include: GlobSet,
    exclude: GlobSet,
}

fn build_matcher(pattern: &str, exclude: &[String]) -> Result<Matcher, String> {
    let include = build_glob_set(&[pattern.to_string()])
        .map_err(|error| format!("invalid glob pattern '{}': {}", pattern, error))?;
    let exclude =
        build_glob_set(exclude).map_err(|error| format!("invalid exclude pattern: {}", error))?;
    Ok(Matcher { include, exclude })
}

/// 编译 glob 集合，`grep` 的 `include` 与本工具的 `pattern` / `exclude` 共用。
///
/// 归一化让两个工具对同一 pattern 给出同一集合：`/` 只匹配真实层级分隔
/// （`literal_separator(true)`），**不含 `/` 的 pattern 自动前置 `**/`**，
/// 因此 `*.rs` 与 `**/*.rs` 等价、都递归匹配，而 `src/*.rs` 只匹配直接子文件。
pub(crate) fn build_glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let normalized = if pattern.contains('/') {
            pattern.clone()
        } else {
            format!("**/{}", pattern)
        };
        let glob = GlobBuilder::new(&normalized)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map_err(|error| error.to_string())?;
        builder.add(glob);
    }
    builder.build().map_err(|error| error.to_string())
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

struct Collected {
    matches: Vec<String>,
    total: usize,
}

/// 同步遍历与匹配，必须在 `spawn_blocking` 内调用。
fn collect_matches(base: &Path, matcher: &Matcher) -> Result<Collected, String> {
    let mut matches: Vec<String> = Vec::new();

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
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let relative = normalize_separators(&relative.to_string_lossy());
        if !matcher.include.is_match(&relative) {
            continue;
        }
        if matcher.exclude.is_match(&relative) {
            continue;
        }
        matches.push(relative);
    }

    matches.sort();
    let total = matches.len();
    Ok(Collected { matches, total })
}

fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}
