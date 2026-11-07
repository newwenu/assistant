use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;
use crate::os::path;
use crate::r#trait::{
    BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier, ValidationResult,
};
use crate::result::ToolResult;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "mkdir";

pub struct AgentMkdirTool;

impl AgentMkdirTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentMkdirTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentMkdirTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Create a directory. Parent directories are created by default (`parents = true`); \
             with `parents = false` a missing parent is an error instead of being created \
             silently. Creating a directory that already exists is a successful no-op. A path \
             that exists as a file is rejected."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path to create" },
                "parents": {
                    "type": "boolean",
                    "description": "Create parent directories (default true)"
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
            read_only: false,
            destructive: false,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        let request = match MkdirRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        match path::resolve(&request.path, context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = MkdirRequest::parse(input)?;
        let resolved = path::resolve(&request.path, context)?;
        Ok(vec![PermissionIntent::new(
            "edit",
            resolved.display().to_string(),
        )])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match MkdirRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let resolve_context = context.clone();
        let created = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve(&request.path, &resolve_context)?;
            let outcome = create_directory(&resolved, request.parents)?;
            Ok::<(PathBuf, MkdirOutcome), String>((resolved, outcome))
        })
        .await;

        let (resolved, outcome) = match created {
            Ok(Ok(created)) => created,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("mkdir task failed: {}", error)),
        };

        let path_text = resolved.display().to_string();
        let assistant_text = if outcome.created {
            format!("Created directory {}.", path_text)
        } else {
            format!("Directory {} already exists; nothing changed.", path_text)
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "created": outcome.created,
                "alreadyExists": !outcome.created
            }),
            assistant_text,
        ))
    }
}

struct MkdirRequest {
    path: String,
    parents: bool,
}

impl MkdirRequest {
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

        let parents = match object.get("parents") {
            None | Some(Value::Null) => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'parents' must be a boolean".to_string()),
        };

        Ok(Self { path, parents })
    }
}

struct MkdirOutcome {
    created: bool,
}

/// 同步创建目录，必须在 `spawn_blocking` 内调用。
///
/// 顺序固定为「查目标 → （非递归时）查父目录 → 创建」：先判定再动手，
/// 避免 `create_dir` 失败后才发现是「已存在目录」这种成功语义。
fn create_directory(path: &Path, parents: bool) -> Result<MkdirOutcome, String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => return Ok(MkdirOutcome { created: false }),
        Ok(_) => {
            return Err(format!(
                "'{}' already exists and is not a directory",
                path.display()
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("cannot access '{}': {}", path.display(), error));
        }
    }

    if parents {
        std::fs::create_dir_all(path).map_err(|error| {
            format!("failed to create directory '{}': {}", path.display(), error)
        })?;
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| format!("'{}' has no parent directory", path.display()))?;
        match std::fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(format!(
                    "parent '{}' is not a directory; cannot create '{}'",
                    parent.display(),
                    path.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "parent directory '{}' does not exist; pass parents = true to create it",
                    parent.display()
                ))
            }
            Err(error) => {
                return Err(format!("cannot access '{}': {}", parent.display(), error));
            }
        }
        match std::fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = std::fs::metadata(path)
                    .map_err(|error| format!("cannot access '{}': {}", path.display(), error))?;
                if !metadata.is_dir() {
                    return Err(format!(
                        "'{}' already exists and is not a directory",
                        path.display()
                    ));
                }
                return Ok(MkdirOutcome { created: false });
            }
            Err(error) => {
                return Err(format!(
                    "failed to create directory '{}': {}",
                    path.display(),
                    error
                ))
            }
        }
    }

    Ok(MkdirOutcome { created: true })
}
