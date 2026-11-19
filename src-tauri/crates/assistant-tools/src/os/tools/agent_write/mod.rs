mod atomic;
mod encoding;
mod lines;
mod preview;

use async_trait::async_trait;
use encoding_rs::Encoding;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::agent_read::file_revision;
use crate::os::path;
use crate::os::text_encoding::{parse_optional_encoding, resolve_label};
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::{
    ReadReceipt, ReadReceiptKey, ReadReceiptStore, DEFAULT_LINE_WINDOW_MAX_CHARS,
};

use atomic::{write_atomically, WriteOutcome};
use encoding::{resolve_target_encoding, TargetEncoding};
use preview::build_preview;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "write";

/// 单次写入上限：按落盘后的完整文件大小计算（`append` 含追加前的内容）。
pub const MAX_WRITE_BYTES: usize = 10 * 1024 * 1024;

/// 转码写入上限（G3）：非 UTF-8 路径必须整文件解码再编码，成本随文件线性增长。
///
/// UTF-8 无 BOM 的直写路径不受此限，仍用 [`MAX_WRITE_BYTES`]。
pub const MAX_TRANSCODE_BYTES: usize = 1024 * 1024;

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
            "Write a text file, creating missing parent directories. Overwrites the \
             existing file unless `append = true`, in which case the content is appended. \
             Writing content identical to the existing file is a no-op (mtime unchanged). \
             Writes are atomic (temp file + fsync + rename) and capped at 10MB. Refuses to \
             overwrite an existing file that cannot be read. Non-UTF-8 writes require a prior \
             read of the file in this session and are capped at 1MB."
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
                "append": { "type": "boolean", "description": "Append mode (default false)" },
                "encoding": {
                    "type": "string",
                    "description": "Optional target encoding (e.g. gbk); omit to keep the file's encoding, new files use UTF-8"
                }
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

        let probe_context = context.clone();
        let raw_path = request.path.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let resolved = path::resolve(&raw_path, &probe_context)?;
            let exists = std::fs::metadata(&resolved)
                .map(|metadata| metadata.is_file())
                .unwrap_or(false);
            Ok::<(PathBuf, bool), String>((resolved, exists))
        })
        .await;

        let (resolved, exists) = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("write task failed: {}", error)),
        };

        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: resolved.display().to_string(),
        };

        let receipt = if exists {
            self.receipts.lookup(&key).await
        } else {
            None
        };
        // 现有文件的编码只作「追加时解码现有内容」的来源；目标编码可以与它不同（显式转码）。
        let source = receipt
            .as_ref()
            .and_then(|receipt| resolve_label(&receipt.encoding));
        let target = resolve_target_encoding(
            request.encoding,
            source,
            receipt.as_ref().is_some_and(|receipt| receipt.has_bom),
        );
        let has_receipt = receipt.is_some();

        let write_path = resolved.clone();
        let content = request.content;
        let append = request.append;
        let written = tokio::task::spawn_blocking(move || {
            let (outcome, target) =
                write_atomically(&write_path, &content, append, target, source, has_receipt)?;
            let revision = file_revision(&write_path)?;
            Ok::<(PathBuf, WriteOutcome, TargetEncoding, String), String>((
                write_path, outcome, target, revision,
            ))
        })
        .await;

        let (resolved, outcome, target, revision) = match written {
            Ok(Ok(written)) => written,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("write task failed: {}", error)),
        };

        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision: revision.clone(),
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: target.name().to_string(),
                    has_bom: target.written_bom(),
                    truncated_lines: 0,
                    line_window_max_chars: DEFAULT_LINE_WINDOW_MAX_CHARS,
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
                "encoding": target.name(),
                "hasBom": target.written_bom(),
                "revision": revision
            }),
            assistant_text,
        ))
    }
}

struct WriteRequest {
    pub(super) path: String,
    pub(super) content: String,
    pub(super) append: bool,
    pub(super) encoding: Option<&'static Encoding>,
}

impl WriteRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

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

        let encoding = parse_optional_encoding(object)?;

        Ok(Self {
            path,
            content,
            append,
            encoding,
        })
    }
}

pub(super) fn too_large_message(bytes: usize) -> String {
    format!(
        "content is {} bytes, exceeding the {} byte write limit",
        bytes, MAX_WRITE_BYTES
    )
}
