mod attrs;
mod exec;
mod scan;
mod target;
mod trash;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::ReadReceiptStore;

use scan::{describe_preview, DELETE_SCAN_LIMIT};
use target::{resolve_target, DeleteRequest};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "delete";

pub struct AgentDeleteTool {
    receipts: Arc<dyn ReadReceiptStore>,
}

impl AgentDeleteTool {
    pub fn new(receipts: Arc<dyn ReadReceiptStore>) -> Self {
        Self { receipts }
    }
}

#[async_trait]
impl Tool for AgentDeleteTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Delete a file, a symlink or a directory. By default the target is moved to the \
             system recycle bin and can be restored from there. Pass `permanent = true` to delete \
             irreversibly — only when the user explicitly asked for permanent deletion. There is \
             no automatic fallback: if the recycle bin is unavailable the call fails instead of \
             deleting permanently. A directory is only removed with `recursive = true`, and a \
             non-empty directory without it is rejected. Links are removed as links; their \
             targets are never entered."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to delete" },
                "recursive": {
                    "type": "boolean",
                    "description": "Delete directory recursively (default false)"
                },
                "permanent": {
                    "type": "boolean",
                    "description": "Permanently delete instead of moving to the system recycle bin (default false). Only set true when the user explicitly asks for irreversible deletion."
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
            destructive: true,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        let request = match DeleteRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return ValidationResult::invalid(error),
        };
        match resolve_target(&request, context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let request = DeleteRequest::parse(input)?;
        let target = resolve_target(&request, context)?;
        let resource = target.display().to_string();
        let mut intents = vec![PermissionIntent::new("delete", resource.clone())];
        if request.permanent {
            intents.push(PermissionIntent::new("purge", resource));
        }
        Ok(intents)
    }

    async fn preview_for_approval(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        let request = DeleteRequest::parse(input).ok()?;
        let recursive = request.recursive;
        let permanent = request.permanent;
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            let target = resolve_target(&request, &context).ok()?;
            describe_preview(&target, recursive, permanent)
        })
        .await
        .ok()?
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match DeleteRequest::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let blocking_context = context.clone();
        let deleted = tokio::task::spawn_blocking(move || {
            let target = match resolve_target(&request, &blocking_context) {
                Ok(target) => target,
                Err(error) => return (None, false, Err(error)),
            };
            let outcome = exec::delete_path(&target, request.recursive, request.permanent);
            let gone = std::fs::symlink_metadata(&target).is_err();
            (Some(target), gone, outcome)
        })
        .await;

        let (target, gone, outcome) = match deleted {
            Ok(value) => value,
            Err(error) => return Err(format!("delete task failed: {}", error)),
        };

        if gone {
            if let Some(target) = target.as_deref() {
                self.receipts.invalidate_path(target).await;
            }
        }

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        let Some(target) = target else {
            return Err("delete resolved no target".to_string());
        };

        let path_text = target.display().to_string();
        let assistant_text = if outcome.is_directory {
            let (entries, bytes) = if outcome.truncated {
                (
                    format!(">={}", DELETE_SCAN_LIMIT),
                    format!(">={}", DELETE_SCAN_LIMIT),
                )
            } else {
                (outcome.entries.to_string(), outcome.bytes.to_string())
            };
            if outcome.permanent {
                format!(
                    "Permanently deleted directory at: {} ({} entries, {} bytes)",
                    path_text, entries, bytes
                )
            } else {
                format!(
                    "Moved directory to the system recycle bin: {} ({} entries, {} bytes)",
                    path_text, entries, bytes
                )
            }
        } else if outcome.permanent {
            format!("Permanently deleted file at: {}", path_text)
        } else {
            format!("Moved file to the system recycle bin: {}", path_text)
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "isDirectory": outcome.is_directory,
                "recursive": outcome.recursive,
                "permanent": outcome.permanent,
                "entriesDeleted": outcome.entries,
                "bytesDeleted": outcome.bytes,
                "truncated": outcome.truncated
            }),
            assistant_text,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::attrs::is_link_like;
    use super::exec::describe_failure;
    use super::scan::scan_tree;
    use super::target::{resolve_target, DeleteRequest};
    use super::trash::trash_failure_message;
    use super::*;
    use crate::storage::InMemoryReadReceiptStore;
    use std::io::ErrorKind;

    /// 失败后重查目标：仍存在 → 「未删除或已部分删除」；已不存在 → 「已完成，但随后失败」。
    #[test]
    fn describe_failure_reports_the_actual_state_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("exists.txt");
        std::fs::write(&existing, "alpha\n").unwrap();
        let error = std::io::Error::new(ErrorKind::PermissionDenied, "denied");

        let message = describe_failure(&existing, &error, "file");
        assert!(message.contains("still exists"), "unexpected: {}", message);
        assert!(message.contains("partially deleted"));

        let missing = dir.path().join("gone.txt");
        let message = describe_failure(&missing, &error, "file");
        assert!(
            message.contains("no longer exists"),
            "unexpected: {}",
            message
        );
        assert!(message.contains("completed before the error"));
        assert!(!message.contains("still exists"));
    }

    /// `permanent` 的解析规则与 `recursive` 同构：缺省 / null 为 false，非布尔即拒绝。
    #[test]
    fn permanent_defaults_to_false_and_rejects_non_booleans() {
        let default = DeleteRequest::parse(&serde_json::json!({ "path": "a.txt" })).unwrap();
        assert!(!default.permanent);
        assert!(!default.recursive);

        let null = DeleteRequest::parse(&serde_json::json!({ "path": "a.txt", "permanent": null }))
            .unwrap();
        assert!(!null.permanent);

        let explicit =
            DeleteRequest::parse(&serde_json::json!({ "path": "a.txt", "permanent": true }))
                .unwrap();
        assert!(explicit.permanent);

        let rejected =
            DeleteRequest::parse(&serde_json::json!({ "path": "a.txt", "permanent": "yes" }));
        assert_eq!(
            rejected.err().unwrap(),
            "'permanent' must be a boolean".to_string()
        );
    }

    /// 回收站失败文案必须与磁盘现状一致：仍存在 → 可能已部分移动；已消失 → 已完成但随后失败。
    #[test]
    fn trash_failure_message_reports_the_actual_state_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("exists.txt");
        std::fs::write(&existing, "alpha\n").unwrap();

        let message = trash_failure_message(&existing, "denied");
        assert!(
            message.contains("failed to move"),
            "unexpected: {}",
            message
        );
        assert!(message.contains("still exists"), "unexpected: {}", message);
        assert!(
            message.contains("partially moved"),
            "unexpected: {}",
            message
        );
        assert!(
            message.contains("permanent = true"),
            "the message must offer the explicit escape hatch: {}",
            message
        );

        let missing = dir.path().join("gone.txt");
        let message = trash_failure_message(&missing, "denied");
        assert!(
            message.contains("no longer exists"),
            "unexpected: {}",
            message
        );
        assert!(
            message.contains("completed before the error"),
            "unexpected: {}",
            message
        );
        assert!(!message.contains("still exists"), "unexpected: {}", message);
    }

    /// 链接（含 Windows junction 的重解析点）不得被当成真实目录下降。
    #[test]
    fn junction_like_entries_are_classified_as_links() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.txt"), "keep\n").unwrap();
        let link = dir.path().join("link");

        if !crate::os::test_support::link::dir_link(&target, &link) {
            eprintln!("directory links unavailable on this host; skipping");
            return;
        }

        let metadata = std::fs::symlink_metadata(&link).unwrap();
        assert!(
            is_link_like(&metadata),
            "a directory link must never be walked as a real directory"
        );
    }

    /// A16：预演与执行共用同一扫描函数，量级数字必须一致。
    ///
    /// 直接比对 `scan_tree` 的返回值与 `execute` 的 `data`，不解析 `summary` 文案。
    /// 用 `permanent = true` 保持零回收站副作用。
    #[tokio::test]
    async fn preview_and_execution_report_the_same_scale() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::create_dir(tree.join("nested")).unwrap();
        std::fs::write(tree.join("a.txt"), b"abc").unwrap();
        std::fs::write(tree.join("nested/b.txt"), b"abcdef").unwrap();

        let context = ToolUseContext {
            workspace_root: Some(dir.path().display().to_string()),
            ..Default::default()
        };
        let input = serde_json::json!({ "path": "tree", "recursive": true, "permanent": true });
        let request = DeleteRequest::parse(&input).unwrap();
        let target = resolve_target(&request, &context).unwrap();
        let scan = scan_tree(&target).expect("scan");

        let result = AgentDeleteTool::new(Arc::new(InMemoryReadReceiptStore::new()))
            .execute(&input, &context)
            .await
            .expect("execute");
        assert!(result.ok, "delete must succeed: {:?}", result.error);

        let data = result.data.expect("delete data");
        assert_eq!(
            data["entriesDeleted"].as_u64().unwrap() as usize,
            scan.entries,
            "审批预演与实际删除必须共用同一份扫描结果"
        );
        assert_eq!(data["bytesDeleted"].as_u64().unwrap(), scan.bytes);
        assert_eq!(data["truncated"].as_bool().unwrap(), scan.truncated);
    }
}

/// Windows 专属：回收站前置筛查（卷根缺 `$Recycle.Bin`、UNC 共享均判否）。
#[cfg(all(test, windows))]
mod windows_tests {
    use super::trash::volume_has_recycle_bin;
    use std::path::Path;

    /// 卷根没有 `$Recycle.Bin` 或落在 UNC 共享上时必须判否（不依赖真实卷）。
    #[test]
    fn a_volume_without_a_recycle_bin_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain-volume");
        std::fs::create_dir(&plain).unwrap();
        assert!(!volume_has_recycle_bin(&plain));

        let with_bin = dir.path().join("volume-with-bin");
        std::fs::create_dir(&with_bin).unwrap();
        std::fs::create_dir(with_bin.join("$Recycle.Bin")).unwrap();
        assert!(volume_has_recycle_bin(&with_bin));

        assert!(!volume_has_recycle_bin(Path::new(r"\\server\share\folder")));
    }
}
