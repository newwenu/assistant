use async_trait::async_trait;
use serde_json::Value;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::path;
use crate::r#trait::{
    BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier, ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::ReadReceiptStore;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "move";

pub struct AgentMoveTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentMoveTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentMoveTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Move or rename a file or directory with a single atomic rename. The target parent \
             directory must already exist (use mkdir). An existing target is rejected unless \
             `overwrite = true`, and overwriting a directory is only allowed when it is empty. \
             Cross-device moves are reported as errors instead of falling back to copy + delete."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Existing path to move" },
                "target": { "type": "string", "description": "Destination path" },
                "overwrite": {
                    "type": "boolean",
                    "description": "Allow replacing an existing target (default false)"
                }
            },
            "required": ["source", "target"]
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
        let request = match MoveRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if request.source == request.target {
            return ValidationResult::invalid(same_path_message(&request.source));
        }
        match resolve_both(&request, context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = MoveRequest::parse(input)?;
        let (source, target) = resolve_both(&request, context)?;
        Ok(vec![
            PermissionIntent::new("edit", source.display().to_string()),
            PermissionIntent::new("edit", target.display().to_string()),
        ])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match MoveRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        if request.source == request.target {
            return Ok(ToolResult::err(same_path_message(&request.source)));
        }

        let resolve_context = context.clone();
        let moved = tokio::task::spawn_blocking(move || {
            let (source, target) = resolve_both(&request, &resolve_context)?;
            let outcome = move_path(&source, &target, request.overwrite)?;
            Ok::<(PathBuf, PathBuf, MoveOutcome), String>((source, target, outcome))
        })
        .await;

        let (source, target, outcome) = match moved {
            Ok(Ok(moved)) => moved,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("move task failed: {}", error)),
        };

        self.receipts.invalidate_path(&source).await;

        let source_text = source.display().to_string();
        let target_text = target.display().to_string();
        let assistant_text = if outcome.overwritten {
            format!(
                "Moved {} to {}, replacing the existing target.",
                source_text, target_text
            )
        } else {
            format!("Moved {} to {}.", source_text, target_text)
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "source": source_text,
                "target": target_text,
                "overwritten": outcome.overwritten,
                "isDirectory": outcome.is_directory
            }),
            assistant_text,
        ))
    }
}

struct MoveRequest {
    source: String,
    target: String,
    overwrite: bool,
}

impl MoveRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let source = object
            .get("source")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing or empty 'source' field".to_string())?
            .to_string();

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing or empty 'target' field".to_string())?
            .to_string();

        let overwrite = match object.get("overwrite") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'overwrite' must be a boolean".to_string()),
        };

        Ok(Self {
            source,
            target,
            overwrite,
        })
    }
}

fn same_path_message(path: &str) -> String {
    format!(
        "'source' and 'target' are the same path ('{}'); nothing to move",
        path
    )
}

/// 两端都走叶子保留解析：移动操作的是链接本身，`canonicalize` 会把链接换成其真实目标。
fn resolve_both(
    request: &MoveRequest,
    context: &ToolUseContext,
) -> Result<(PathBuf, PathBuf), String> {
    let source = path::resolve_leaf_preserving(&request.source, context)?;
    let target = path::resolve_leaf_preserving(&request.target, context)?;
    Ok((source, target))
}

struct MoveOutcome {
    overwritten: bool,
    is_directory: bool,
}

/// 同步移动，必须在 `spawn_blocking` 内调用。
///
/// 顺序固定为「查源 → 查目标 → （按需）删目标 → 查目标父目录 → rename」：
/// 所有拒绝都发生在任何破坏性动作之前，`overwrite` 的删除是最后一步的前置动作。
fn move_path(source: &Path, target: &Path, overwrite: bool) -> Result<MoveOutcome, String> {
    let source_meta = match std::fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(format!("'{}' does not exist", source.display()))
        }
        Err(error) => {
            return Err(format!("cannot access '{}': {}", source.display(), error));
        }
    };
    let is_directory = source_meta.file_type().is_dir();

    let mut overwritten = false;
    match std::fs::symlink_metadata(target) {
        Ok(metadata) => {
            if !overwrite {
                return Err(format!(
                    "'{}' already exists; pass overwrite = true to replace it",
                    target.display()
                ));
            }
            if metadata.file_type().is_dir() {
                ensure_empty_directory(target)?;
            }
            remove_target(target, &metadata)?;
            overwritten = true;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("cannot access '{}': {}", target.display(), error));
        }
    }

    let parent = target
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", target.display()))?;
    match std::fs::metadata(parent) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(format!(
                "target parent '{}' is not a directory",
                parent.display()
            ))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(format!(
                "target parent directory '{}' does not exist; create it with mkdir first",
                parent.display()
            ))
        }
        Err(error) => {
            return Err(format!("cannot access '{}': {}", parent.display(), error));
        }
    }

    std::fs::rename(source, target).map_err(|error| {
        if error.kind() == ErrorKind::CrossesDevices {
            format!(
                "cannot move '{}' to '{}' across devices/filesystems; copy the content and then \
                 delete the source in two separate steps",
                source.display(),
                target.display()
            )
        } else {
            format!(
                "failed to move '{}' to '{}': {}",
                source.display(),
                target.display(),
                error
            )
        }
    })?;

    Ok(MoveOutcome {
        overwritten,
        is_directory,
    })
}

/// 非空目录拒绝：`move` 不得隐式递归删除，那会让一次审批覆盖两类破坏性改动。
fn ensure_empty_directory(path: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|error| format!("cannot read '{}': {}", path.display(), error))?;
    if entries.next().is_some() {
        return Err(format!(
            "'{}' is a non-empty directory; delete its contents with the delete tool first \
             (move never deletes recursively)",
            path.display()
        ));
    }
    Ok(())
}

/// 删除被覆盖的目标。链接按链接删除，不跟随到其目标。
fn remove_target(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        // Windows 上目录链接 / junction 只能由 `remove_dir` 删除，文件链接只能由 `remove_file` 删除。
        if std::fs::remove_file(path).is_ok() || std::fs::remove_dir(path).is_ok() {
            return Ok(());
        }
        return Err(format!(
            "failed to remove the existing target '{}'",
            path.display()
        ));
    }
    if file_type.is_dir() {
        std::fs::remove_dir(path)
            .map_err(|error| format!("failed to remove '{}': {}", path.display(), error))
    } else {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to remove '{}': {}", path.display(), error))
    }
}
