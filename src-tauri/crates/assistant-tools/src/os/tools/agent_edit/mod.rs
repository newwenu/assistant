mod apply;
mod locate;
mod normalize;
mod report;
mod text;

use async_trait::async_trait;
use encoding_rs::UTF_8;
use serde_json::Value;
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::path;
use crate::os::text_encoding::resolve_label;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::{ReadReceipt, ReadReceiptKey, ReadReceiptStore};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "edit";

/// 歧义错误里最多展示的上下文段数。
pub const MAX_MATCH_CONTEXTS: usize = 5;

/// 上下文段 / 最近片段的前后行数。
pub const CONTEXT_LINES: usize = 2;

/// 归一化匹配尝试的 tab 宽度：4 空格优先，其次 2 空格。
pub const TAB_WIDTHS: [usize; 2] = [4, 2];

/// diff 与预演的每侧最大行数。
pub const MAX_DIFF_LINES: usize = 20;

pub struct AgentEditTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentEditTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentEditTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Replace an exact fragment of an existing text file. `old_string` must be \
             copied from the latest read output (without the line-number prefix) and match \
             exactly once unless `replace_all = true`. Matching degrades progressively \
             (exact, stripped line prefixes, trailing newline, trailing whitespace, leading \
             whitespace, normalized) and stops at the first pass that hits; multiple hits in \
             that pass are reported as ambiguous instead of picking the first. The file must \
             have been read (or written) in this session and must not have changed since; its \
             encoding and BOM come from that read, so non-UTF-8 files are edited in place. \
             If that read had lines cut at the line-width limit, the edit is rejected: the \
             unseen tail of those lines cannot be verified (use `grep` to locate the exact \
             line first). \
             Writes are atomic (temp file + fsync + rename); CRLF files stay CRLF."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to edit" },
                "old_string": {
                    "type": "string",
                    "description": "Exact current text to replace, copied from the latest read output without the line-number prefix"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text; may be empty to delete the fragment. Keep the indentation style of old_string"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace every match instead of requiring a unique match (default false)"
                }
            },
            "required": ["path", "old_string", "new_string"]
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
        let request = match EditRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        if let Some(error) = rejected_arguments(&request) {
            return ValidationResult::invalid(error);
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
        let request = EditRequest::parse(input)?;
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
        let request = EditRequest::parse(input).ok()?;
        let preview_context = context.clone();
        tokio::task::spawn_blocking(move || report::build_preview(&request, &preview_context))
            .await
            .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match EditRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        if let Some(error) = rejected_arguments(&request) {
            return Ok(ToolResult::err(error));
        }

        let probe_context = context.clone();
        let probe_request = request.clone();
        let probed = tokio::task::spawn_blocking(move || {
            apply::probe_target(&probe_request, &probe_context)
        })
        .await;
        let (resolved, revision) = match probed {
            Ok(Ok(probed)) => probed,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("edit task failed: {}", error)),
        };

        let path_text = resolved.display().to_string();
        let key = ReadReceiptKey {
            session_id: context.session_id.clone(),
            path: path_text.clone(),
        };
        let receipt = match self.receipts.lookup(&key).await {
            None => {
                return Ok(ToolResult::err(format!(
                    "'{}' has not been read in this session; call read on the whole file before \
                     editing (blind edits can overwrite changes made outside this session)",
                    path_text
                )))
            }
            Some(receipt) if receipt.revision != revision => {
                return Ok(ToolResult::err(format!(
                    "'{}' changed after it was last read in this session (revision {} -> {}); \
                     call read again before editing",
                    path_text, receipt.revision, revision
                )))
            }
            Some(receipt) => receipt,
        };

        // I57 §3.6 闸门：回执上有行被切时，被切掉的尾部无法与 `old_string` 逐字校验，
        // 因此拒绝写入。只读回执、不重扫文件，也不给模型开关（输入 schema 不变）。
        if receipt.truncated_lines > 0 {
            return Ok(ToolResult::err(format!(
                "'{}' was read with {} line(s) cut at {} chars; the unseen part of those lines \
                 cannot be verified before editing (use 'grep' to locate the exact line first)",
                path_text, receipt.truncated_lines, receipt.line_window_max_chars
            )));
        }

        let encoding = resolve_label(&receipt.encoding).unwrap_or(UTF_8);
        let has_bom = receipt.has_bom;
        // I58 §3.6：定位失败的诊断是否附「可申请 `super_edit`」的指引。
        // 门控 `request_tool`（唯一申请入口），不是 `super_edit` 是否已在工具集里。
        let escalation = context
            .available_tool_names
            .as_ref()
            .is_some_and(|names| names.iter().any(|name| name == "request_tool"));
        let edit_request = request.clone();
        let edit_path = resolved.clone();
        let edited = tokio::task::spawn_blocking(move || {
            apply::apply_edit(&edit_path, &edit_request, encoding, has_bom, escalation)
        })
        .await;
        let write = match edited {
            Ok(Ok(write)) => write,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("edit task failed: {}", error)),
        };
        let outcome = write.outcome;

        self.receipts
            .record(
                key,
                ReadReceipt {
                    revision: write.revision.clone(),
                    tool_call_id: context.tool_call_id.clone().unwrap_or_default(),
                    encoding: write.encoding.name().to_string(),
                    has_bom: write.has_bom,
                    truncated_lines: 0,
                    line_window_max_chars: receipt.line_window_max_chars,
                },
            )
            .await;

        let assistant_text = format!(
            "Edited {}: {} replacement(s) at line {} (lines {} -> {}).\n{}",
            path_text,
            outcome.match_count,
            outcome.start_line,
            outcome.old_end_line,
            outcome.new_end_line,
            outcome.diff
        );

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "startLine": outcome.start_line,
                "oldEndLine": outcome.old_end_line,
                "newEndLine": outcome.new_end_line,
                "matchCount": outcome.match_count,
                "tier": outcome.tier,
                "diff": outcome.diff,
                "revision": write.revision,
                "encoding": write.encoding.name(),
                "hasBom": write.has_bom
            }),
            assistant_text,
        ))
    }
}

#[derive(Clone)]
struct EditRequest {
    path: String,
    old_string: String,
    new_string: String,
    replace_all: bool,
}

impl EditRequest {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

        let old_string = object
            .get("old_string")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing or non-string 'old_string' field".to_string())?
            .to_string();

        let new_string = object
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing or non-string 'new_string' field".to_string())?
            .to_string();

        let replace_all = match object.get("replace_all") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'replace_all' must be a boolean".to_string()),
        };

        Ok(Self {
            path,
            old_string,
            new_string,
            replace_all,
        })
    }
}

/// 档 0：无意义写回的前置拒绝。纯参数判定，不触碰文件系统。
fn rejected_arguments(request: &EditRequest) -> Option<String> {
    if request.old_string.is_empty() {
        return Some("'old_string' must not be empty".to_string());
    }
    if request.old_string == request.new_string {
        return Some("'old_string' and 'new_string' are identical; nothing to change".to_string());
    }
    None
}
