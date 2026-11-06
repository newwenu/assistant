use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;
use crate::os::path;
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::ToolResult;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "ls";

/// `depth` 缺省值。
pub const DEFAULT_DEPTH: u64 = 1;
/// `depth` 上限，超出按上限夹取并显式提示。
pub const MAX_DEPTH: u64 = 10;
/// **每层**条目上限：避免深层条目把浅层条目挤掉。
pub const MAX_ENTRIES_PER_DIR: usize = 1000;

pub struct AgentListTool;

impl AgentListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentListTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "List a directory as an indented tree. `depth` defaults to 1 and is capped at 10; \
             entries are sorted by name and directories end with '/'. Each directory returns at \
             most 1000 entries with an explicit truncation notice. `all = true` includes hidden \
             entries. Symbolic links are listed but never followed. Does not read `.gitignore`."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path to list" },
                "depth": { "type": "integer", "description": "Max recursion depth (default 1, max 10)" },
                "all": { "type": "boolean", "description": "Include hidden entries" }
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
        match ListRequest::parse(input) {
            Ok(request) => match path::resolve(&request.path, context) {
                Ok(_) => ValidationResult::ok(),
                Err(error) => ValidationResult::invalid(error),
            },
            Err(error) => ValidationResult::invalid(error),
        }
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match ListRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let resolve_context = context.clone();
        let raw_path = request.path.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve(&raw_path, &resolve_context)?;
            let metadata = std::fs::metadata(&resolved)
                .map_err(|error| format!("failed to stat '{}': {}", resolved.display(), error))?;
            if !metadata.is_dir() {
                return Err(format!(
                    "'{}' is not a directory; use read to read a file",
                    resolved.display()
                ));
            }
            Ok::<PathBuf, String>(resolved)
        })
        .await;

        let resolved = match prepared {
            Ok(Ok(resolved)) => resolved,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("ls task failed: {}", error)),
        };

        let depth = request.depth;
        let depth_clamped = request.depth_clamped;
        let all = request.all;
        let walk_path = resolved.clone();
        let walk =
            tokio::task::spawn_blocking(move || walk(&walk_path, depth, depth_clamped, all)).await;

        let outcome = match walk {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("ls task failed: {}", error)),
        };

        let assistant_text = if outcome.lines.is_empty() {
            "(no entries)".to_string()
        } else {
            outcome.lines.join("\n")
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": resolved.display().to_string(),
                "depth": depth,
                "depthClamped": request.depth_clamped,
                "all": all,
                "totalEntries": outcome.total_entries,
                "truncated": outcome.truncated,
                "entries": outcome.entries
            }),
            assistant_text,
        ))
    }
}

struct ListRequest {
    path: String,
    depth: u64,
    depth_clamped: bool,
    all: bool,
}

impl ListRequest {
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

        let requested_depth = match object.get("depth") {
            None | Some(Value::Null) => DEFAULT_DEPTH,
            Some(value) => match value.as_u64() {
                Some(0) => return Err("'depth' must be >= 1".to_string()),
                Some(depth) => depth,
                None => return Err("'depth' must be a non-negative integer".to_string()),
            },
        };

        let all = match object.get("all") {
            None | Some(Value::Null) => false,
            Some(value) => value
                .as_bool()
                .ok_or_else(|| "'all' must be a boolean".to_string())?,
        };

        let depth_clamped = requested_depth > MAX_DEPTH;
        Ok(Self {
            path,
            depth: requested_depth.min(MAX_DEPTH),
            depth_clamped,
            all,
        })
    }
}

struct WalkOutcome {
    lines: Vec<String>,
    entries: Vec<Value>,
    total_entries: usize,
    truncated: bool,
}

/// 同步目录遍历，必须在 `spawn_blocking` 内调用。
fn walk(
    base: &Path,
    max_depth: u64,
    depth_clamped: bool,
    all: bool,
) -> Result<WalkOutcome, String> {
    let mut outcome = WalkOutcome {
        lines: Vec::new(),
        entries: Vec::new(),
        total_entries: 0,
        truncated: false,
    };

    if depth_clamped {
        outcome
            .lines
            .push(format!("[depth clamped to {}]", MAX_DEPTH));
    }

    walk_dir(base, base, 1, max_depth, all, &mut outcome)?;
    Ok(outcome)
}

fn walk_dir(
    base: &Path,
    dir: &Path,
    depth: u64,
    max_depth: u64,
    all: bool,
    outcome: &mut WalkOutcome,
) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|error| format!("failed to list '{}': {}", dir.display(), error))?;

    let mut candidates: Vec<(String, PathBuf, bool, bool)> = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                return Err(format!(
                    "failed to read an entry of '{}': {}",
                    dir.display(),
                    error
                ))
            }
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if !all && name.starts_with('.') {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                return Err(format!(
                    "failed to stat '{}': {}",
                    entry.path().display(),
                    error
                ))
            }
        };
        let is_link = file_type.is_symlink();
        let is_dir = if is_link {
            entry
                .path()
                .metadata()
                .map(|meta| meta.is_dir())
                .unwrap_or(false)
        } else {
            file_type.is_dir()
        };
        candidates.push((name, entry.path(), is_dir, is_link));
    }

    candidates.sort_by(|left, right| left.0.cmp(&right.0));

    let limit = MAX_ENTRIES_PER_DIR;
    let total = candidates.len();
    let truncated = total > limit;
    if truncated {
        outcome.truncated = true;
    }

    let indent = "  ".repeat((depth - 1) as usize);
    for (name, child, is_dir, is_link) in candidates.into_iter().take(limit) {
        let displayed = if is_dir {
            format!("{}/", name)
        } else {
            name.clone()
        };
        outcome.lines.push(format!("{}{}", indent, displayed));
        outcome.total_entries += 1;
        let relative = relative_path(base, &child);
        outcome.entries.push(serde_json::json!({
            "path": relative,
            "name": name,
            "isDirectory": is_dir,
            "isSymlink": is_link
        }));

        if !is_dir || is_link || depth >= max_depth {
            continue;
        }
        if !descends_within(base, &child) {
            continue;
        }
        walk_dir(base, &child, depth + 1, max_depth, all, outcome)?;
    }

    if truncated {
        outcome.lines.push(format!(
            "{}[truncated: showing {} of {} entries]",
            indent, limit, total
        ));
    }
    Ok(())
}

/// 链接（含 junction）不得把遍历带出基准目录。
fn descends_within(base: &Path, child: &Path) -> bool {
    match path::canonicalize_lenient(child) {
        Ok(canonical) => path::is_component_prefix(&canonical, base),
        Err(_) => false,
    }
}

fn relative_path(base: &Path, child: &Path) -> String {
    child
        .strip_prefix(base)
        .map(|relative| relative.to_string_lossy().to_string())
        .unwrap_or_else(|_| child.display().to_string())
}
