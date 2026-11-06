use async_trait::async_trait;
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::agent_read::file_revision;
use crate::os::output::human_bytes;
use crate::os::path;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::{ReadReceipt, ReadReceiptKey, ReadReceiptStore};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "write";

/// 单次写入上限：按落盘后的完整文件大小计算（`append` 含追加前的内容）。
pub const MAX_WRITE_BYTES: usize = 10 * 1024 * 1024;

pub struct AgentWriteTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentWriteTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentWriteTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Write a UTF-8 text file, creating missing parent directories. Overwrites the \
             existing file unless `append = true`, in which case the content is appended. \
             Writing content identical to the existing file is a no-op (mtime unchanged). \
             Writes are atomic (temp file + fsync + rename) and capped at 10MB. Refuses to \
             overwrite an existing file that cannot be read."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to write" },
                "content": {
                    "type": "string",
                    "description": "Full file content, or the fragment to append"
                },
                "append": { "type": "boolean", "description": "Append mode (default false)" }
            },
            "required": ["path", "content"]
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
        let request = match WriteRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if request.content.len() > MAX_WRITE_BYTES {
            return ValidationResult::invalid(too_large_message(request.content.len()));
        }
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
        let request = WriteRequest::parse(input)?;
        let resolved = path::resolve(&request.path, context)?;
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
        let request = WriteRequest::parse(input).ok()?;
        let preview_context = context.clone();
        tokio::task::spawn_blocking(move || build_preview(&request, &preview_context))
            .await
            .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match WriteRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        if request.content.len() > MAX_WRITE_BYTES {
            return Ok(ToolResult::err(too_large_message(request.content.len())));
        }

        let resolve_context = context.clone();
        let written = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve(&request.path, &resolve_context)?;
            let outcome = write_atomically(&resolved, &request.content, request.append)?;
            let revision = file_revision(&resolved)?;
            Ok::<(PathBuf, WriteOutcome, String), String>((resolved, outcome, revision))
        })
        .await;

        let (resolved, outcome, revision) = match written {
            Ok(Ok(written)) => written,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("write task failed: {}", error)),
        };

        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: resolved.display().to_string(),
            start_line: None,
            end_line: None,
        };
        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision: revision.clone(),
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                },
            )
            .await;

        let path_text = resolved.display().to_string();
        let assistant_text = if outcome.unchanged {
            format!(
                "{} already has identical content; nothing written.",
                path_text
            )
        } else if outcome.created {
            format!(
                "Created {} ({} lines, {} bytes).",
                path_text, outcome.lines, outcome.bytes
            )
        } else if outcome.appended {
            format!(
                "Appended to {} (now {} lines, {} bytes).",
                path_text, outcome.lines, outcome.bytes
            )
        } else {
            format!(
                "Wrote {} ({} lines, {} bytes).",
                path_text, outcome.lines, outcome.bytes
            )
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "created": outcome.created,
                "appended": outcome.appended,
                "unchanged": outcome.unchanged,
                "lines": outcome.lines,
                "bytes": outcome.bytes,
                "revision": revision
            }),
            assistant_text,
        ))
    }
}

struct WriteRequest {
    path: String,
    content: String,
    append: bool,
}

impl WriteRequest {
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

        let content = object
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing or non-string 'content' field".to_string())?
            .to_string();

        let append = match object.get("append") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'append' must be a boolean".to_string()),
        };

        Ok(Self {
            path,
            content,
            append,
        })
    }
}

/// 一次写入的结果。`unchanged` 为真时不写盘，其余字段描述写盘后的最终文件。
struct WriteOutcome {
    created: bool,
    appended: bool,
    unchanged: bool,
    bytes: usize,
    lines: usize,
}

/// 同步写入，必须在 `spawn_blocking` 内调用。
///
/// 顺序固定为「读现有内容 → 判等 / 拼装 → 建父目录 → 临时文件 → `sync_all` → rename」：
/// 先读是为了在不可读时提前拒绝覆盖，而不是写完才发现目标被占用。
fn write_atomically(path: &Path, content: &str, append: bool) -> Result<WriteOutcome, String> {
    let existing = read_existing(path)?;
    let created = existing.is_none();

    let payload: Vec<u8> = match &existing {
        Some(bytes) if append => {
            let mut combined = bytes.clone();
            combined.extend_from_slice(content.as_bytes());
            combined
        }
        _ => content.as_bytes().to_vec(),
    };

    if let Some(bytes) = &existing {
        if bytes == &payload {
            return Ok(WriteOutcome {
                created: false,
                appended: false,
                unchanged: true,
                bytes: bytes.len(),
                lines: count_lines(bytes),
            });
        }
    }

    if payload.len() > MAX_WRITE_BYTES {
        return Err(too_large_message(payload.len()));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create parent directory '{}': {}",
            parent.display(),
            error
        )
    })?;

    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "failed to create a temporary file in '{}': {}",
            parent.display(),
            error
        )
    })?;
    temp.write_all(&payload)
        .map_err(|error| format!("failed to write '{}': {}", path.display(), error))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("failed to flush '{}': {}", path.display(), error))?;
    temp.persist(path)
        .map_err(|error| format!("failed to replace '{}': {}", path.display(), error.error))?;

    Ok(WriteOutcome {
        created,
        appended: append && !created,
        unchanged: false,
        bytes: payload.len(),
        lines: count_lines(&payload),
    })
}

/// 读取现有文件内容：不存在返回 `None`。
///
/// 已存在但读不到（权限 / 被占用）一律拒绝：无法确认现有内容时覆盖不可逆。
fn read_existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to stat '{}': {}", path.display(), error)),
    };

    if metadata.is_dir() {
        return Err(format!(
            "'{}' is a directory; use mkdir to create directories",
            path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("'{}' is not a regular file", path.display()));
    }

    std::fs::read(path).map(Some).map_err(|error| {
        format!(
            "'{}' exists but cannot be read ({}); refusing to overwrite an unreadable file — \
             check its permissions or close the process holding it",
            path.display(),
            error
        )
    })
}

fn too_large_message(bytes: usize) -> String {
    format!(
        "content is {} bytes, exceeding the {} byte write limit; split the file into smaller writes",
        bytes, MAX_WRITE_BYTES
    )
}

/// 按字节计行数，与 `str::lines()` 的口径一致（空文件 0 行，末尾换行不多算一行）。
fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        newlines
    } else {
        newlines + 1
    }
}

/// 审批预演：只取 `metadata` 与流式行计数，不把内容读进内存。
fn build_preview(request: &WriteRequest, context: &ToolUseContext) -> Option<ApprovalPreview> {
    let resolved = path::resolve(&request.path, context).ok()?;
    let existing = std::fs::metadata(&resolved)
        .ok()
        .filter(|metadata| metadata.is_file());

    Some(match existing {
        Some(metadata) => {
            let lines = count_lines_in_file(&resolved)?;
            let verb = if request.append {
                "追加到"
            } else {
                "覆盖"
            };
            ApprovalPreview {
                summary: format!(
                    "{} 1 个现有文件（当前 {} 行 / {}）",
                    verb,
                    lines,
                    human_bytes(metadata.len() as usize)
                ),
                truncated: false,
                estimated: false,
            }
        }
        None => ApprovalPreview {
            summary: format!(
                "新建文件（{} 行 / {}）",
                count_lines(request.content.as_bytes()),
                human_bytes(request.content.len())
            ),
            truncated: false,
            estimated: false,
        },
    })
}

/// 流式统计行数：逐块读到换行为止，不保留内容。
fn count_lines_in_file(path: &Path) -> Option<usize> {
    use std::io::BufRead;

    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = Vec::new();
    let mut lines = 0usize;
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => return Some(lines),
            Ok(_) => lines += 1,
            Err(_) => return None,
        }
    }
}
