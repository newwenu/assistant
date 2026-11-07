use async_trait::async_trait;
use serde_json::Value;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::context::ToolUseContext;
use crate::os::output::human_bytes;
use crate::os::path;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::storage::ReadReceiptStore;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "delete";

/// 预扫描条目上限：达到即停止并标记 `truncated`，但**不**因此拒绝删除。
const DELETE_SCAN_LIMIT: usize = 100_000;

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
            "Delete a file, a symlink or a directory. Deletion is irreversible: there is no \
             recycle bin, no backup and no `force` switch. A directory is only deleted with \
             `recursive = true`, and a non-empty directory without it is rejected. Links are \
             removed as links, their targets are never entered."
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
        Ok(vec![PermissionIntent::new(
            "delete",
            target.display().to_string(),
        )])
    }

    async fn preview_for_approval(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        let request = DeleteRequest::parse(input).ok()?;
        let recursive = request.recursive;
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            let target = resolve_target(&request, &context).ok()?;
            describe_preview(&target, recursive)
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
            let target = resolve_target(&request, &blocking_context)?;
            let outcome = delete_path(&target, request.recursive)?;
            Ok::<(PathBuf, DeleteOutcome), String>((target, outcome))
        })
        .await;

        let (target, outcome) = match deleted {
            Ok(Ok(deleted)) => deleted,
            Ok(Err(error)) => return Ok(ToolResult::err(error)),
            Err(error) => return Err(format!("delete task failed: {}", error)),
        };

        self.receipts.invalidate_path(&target).await;

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
            format!(
                "Deleted directory at: {} ({} entries, {} bytes)",
                path_text, entries, bytes
            )
        } else {
            format!("Deleted file at: {}", path_text)
        };

        Ok(ToolResult::ok_with_assistant(
            serde_json::json!({
                "path": path_text,
                "isDirectory": outcome.is_directory,
                "recursive": outcome.recursive,
                "entriesDeleted": outcome.entries,
                "bytesDeleted": outcome.bytes,
                "truncated": outcome.truncated
            }),
            assistant_text,
        ))
    }
}

struct DeleteRequest {
    path: String,
    recursive: bool,
}

impl DeleteRequest {
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

        let recursive = match object.get("recursive") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'recursive' must be a boolean".to_string()),
        };

        Ok(Self { path, recursive })
    }
}

/// 解析删除目标。
///
/// 用 `resolve_leaf_preserving` 而不是 `resolve`：`resolve` 会把叶子上的符号链接换成真实
/// 目标，删除就变成「删目标」而不是「删链接」（delete §3.2 明令禁止）。
/// 父目录仍按 `canonicalize_lenient` 归一，`..` 穿越、受保护目录与 workspace 边界照常生效。
fn resolve_target(request: &DeleteRequest, context: &ToolUseContext) -> Result<PathBuf, String> {
    let raw = Path::new(&request.path);
    if raw.parent().is_none() {
        return Err(format!(
            "refusing to delete the filesystem root '{}'",
            request.path
        ));
    }
    let target = path::resolve_leaf_preserving(&request.path, context)?;
    reject_undeletable(&target, context)?;
    Ok(target)
}

/// 不可删目标：文件系统根、`workspace_root` 本身、家目录本身。
///
/// 只拒家目录本身，不拒其子项（工作区通常就在家目录下）。凭据目录与系统目录由
/// `os::path` 的 `reject_system_dirs` 在解析阶段拒绝，这里不重复判定。
///
/// 每个候选都同时检查「叶子保留路径」与「跟随链接后的真实路径」：指向工作区根 / 家目录
/// 的链接同样拒绝（delete §3.1 的「链接形式」），删除链接虽然只删链接，但没有理由允许
/// 模型对这类目标下手。
fn reject_undeletable(target: &Path, context: &ToolUseContext) -> Result<(), String> {
    let followed = path::canonicalize_lenient(target).ok();
    let candidates = std::iter::once(target.to_path_buf()).chain(followed);

    for candidate in candidates {
        if candidate.parent().is_none() {
            return Err(format!(
                "refusing to delete the filesystem root '{}'",
                candidate.display()
            ));
        }
        if let Some(root) = context.workspace_root.as_deref() {
            if let Ok(root) = path::canonicalize_lenient(Path::new(root)) {
                if same_path(&candidate, &root) {
                    return Err(format!(
                        "refusing to delete the workspace root itself ('{}')",
                        root.display()
                    ));
                }
            }
        }
        if let Some(home) = home_dir() {
            if let Ok(home) = path::canonicalize_lenient(&home) {
                if same_path(&candidate, &home) {
                    return Err(format!(
                        "refusing to delete the home directory itself ('{}')",
                        home.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// 组件级相等（Windows 大小写不敏感由 `is_component_prefix` 承担）。
fn same_path(left: &Path, right: &Path) -> bool {
    path::is_component_prefix(left, right) && path::is_component_prefix(right, left)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// 目标文件是否带只读属性。
///
/// Windows 上 `std::fs::remove_file` 在 `DeleteFileW` 失败后会**自动清掉**只读属性再重试，
/// 那等于「删除顺手改了属性」——正是 delete §3.4 明确禁止的额外破坏性写。为了保持
/// 「删只读文件 → `Business` + 提示先移除属性」的确定性，这里改成前置拒绝：
/// 不在删除路径里改任何属性，只把选择权交回用户。
fn is_readonly(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        metadata.file_attributes() & FILE_ATTRIBUTE_READONLY != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn readonly_message(target: &Path) -> String {
    format!(
        "'{}' is read-only; remove the read-only attribute first (delete never changes attributes)",
        target.display()
    )
}

/// 是否「链接类」条目（Unix symlink、Windows symlink 与 junction）。
///
/// Windows 的 junction 在 `symlink_metadata` 里表现为**目录 + 重解析点**，`is_symlink()`
/// 为 false；只靠 `is_dir()` 会把 junction 当成真实目录去递归，等于删掉它的目标内容
/// （delete §3.2 明令禁止）。因此这里额外看 `FILE_ATTRIBUTE_REPARSE_POINT`。
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

struct ScanOutcome {
    entries: usize,
    bytes: u64,
    truncated: bool,
}

/// 递归预扫描：显式栈 + `read_dir`，不下降链接。
///
/// 只服务两件事：非空目录判定与量级文案。**不承担安全职责**——边界已由 `resolve` 一次性
/// 确定，子树也无法借链接逃逸（链接不下降）。`preview_for_approval` 与 `execute` 共用
/// 这一份实现，避免「审批说 3 项、实际删 300 项」。
fn scan_tree(root: &Path) -> Result<ScanOutcome, String> {
    let mut entries = 0usize;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let reader = std::fs::read_dir(&directory).map_err(|error| {
            format!(
                "cannot read '{}' to confirm what would be deleted: {}",
                directory.display(),
                error
            )
        })?;
        for entry in reader {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot read '{}' to confirm what would be deleted: {}",
                    directory.display(),
                    error
                )
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| format!("cannot inspect '{}': {}", path.display(), error))?;
            let file_type = metadata.file_type();
            entries += 1;
            if is_link_like(&metadata) {
                // 链接本身不计字节：删除它不回收目标占用的空间，也不进入目标。
            } else if file_type.is_dir() {
                stack.push(path);
            } else {
                bytes = bytes.saturating_add(metadata.len());
            }
            if entries >= DELETE_SCAN_LIMIT {
                return Ok(ScanOutcome {
                    entries,
                    bytes,
                    truncated: true,
                });
            }
        }
    }

    Ok(ScanOutcome {
        entries,
        bytes,
        truncated: false,
    })
}

fn describe_preview(target: &Path, recursive: bool) -> Option<ApprovalPreview> {
    let metadata = std::fs::symlink_metadata(target).ok()?;
    let file_type = metadata.file_type();

    if is_link_like(&metadata) {
        return Some(ApprovalPreview {
            summary: format!(
                "delete the link at {} (its target is not touched)",
                target.display()
            ),
            truncated: false,
            estimated: true,
        });
    }
    if !file_type.is_dir() {
        return Some(ApprovalPreview {
            summary: format!(
                "delete the file at {} ({})",
                target.display(),
                human_bytes(metadata.len() as usize)
            ),
            truncated: false,
            estimated: true,
        });
    }

    let scan = scan_tree(target).ok()?;
    let entries = if scan.truncated {
        format!("at least {} entries", DELETE_SCAN_LIMIT)
    } else {
        format!("{} entries", scan.entries)
    };
    let bytes = if scan.truncated {
        format!("at least {} bytes", DELETE_SCAN_LIMIT)
    } else {
        human_bytes(scan.bytes as usize)
    };
    let verb = if recursive {
        "delete the directory recursively at"
    } else {
        "delete the directory at"
    };
    Some(ApprovalPreview {
        summary: format!("{} {} ({}, {})", verb, target.display(), entries, bytes),
        truncated: scan.truncated,
        estimated: true,
    })
}

struct DeleteOutcome {
    is_directory: bool,
    recursive: bool,
    entries: usize,
    bytes: u64,
    truncated: bool,
}

/// 同步删除，必须在 `spawn_blocking` 内调用。
///
/// 分类只用 `symlink_metadata`（不跟随链接），所有拒绝都发生在任何删除动作之前。
fn delete_path(target: &Path, recursive: bool) -> Result<DeleteOutcome, String> {
    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(format!("'{}' does not exist", target.display()))
        }
        Err(error) => {
            return Err(format!("cannot access '{}': {}", target.display(), error));
        }
    };
    let file_type = metadata.file_type();

    if is_link_like(&metadata) {
        remove_link(target)?;
        return Ok(DeleteOutcome {
            is_directory: false,
            recursive,
            entries: 1,
            bytes: 0,
            truncated: false,
        });
    }

    if file_type.is_dir() {
        if !recursive {
            ensure_empty_directory(target)?;
            std::fs::remove_dir(target).map_err(|error| {
                let mut message = describe_failure(target, &error, "directory");
                message.push_str("; if the directory is no longer empty, pass recursive = true");
                message
            })?;
            return Ok(DeleteOutcome {
                is_directory: true,
                recursive,
                entries: 0,
                bytes: 0,
                truncated: false,
            });
        }
        let scan = scan_tree(target)?;
        std::fs::remove_dir_all(target)
            .map_err(|error| describe_failure(target, &error, "directory"))?;
        return Ok(DeleteOutcome {
            is_directory: true,
            recursive,
            entries: scan.entries,
            bytes: scan.bytes,
            truncated: scan.truncated,
        });
    }

    if is_readonly(&metadata) {
        return Err(readonly_message(target));
    }
    let bytes = metadata.len();
    std::fs::remove_file(target).map_err(|error| describe_failure(target, &error, "file"))?;
    Ok(DeleteOutcome {
        is_directory: false,
        recursive,
        entries: 1,
        bytes,
        truncated: false,
    })
}

/// 链接一律按链接删除：删链接本身、不进入目标，`recursive` 不改变行为。
///
/// Windows 上目录链接 / junction 是目录条目，`remove_file` 会失败，需要 `remove_dir` 兜底。
fn remove_link(target: &Path) -> Result<(), String> {
    if std::fs::remove_file(target).is_ok() || std::fs::remove_dir(target).is_ok() {
        return Ok(());
    }
    Err(format!(
        "failed to remove the link at '{}'",
        target.display()
    ))
}

/// 空目录判定只读第一项，不整目录列举。
fn ensure_empty_directory(target: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(target).map_err(|error| {
        format!(
            "cannot read '{}' to confirm what would be deleted: {}",
            target.display(),
            error
        )
    })?;
    if entries.next().is_some() {
        return Err(format!(
            "'{}' is a non-empty directory; pass recursive = true to delete it",
            target.display()
        ));
    }
    Ok(())
}

/// 失败文案必须与磁盘现状一致：不得谎报成功，也不得谎报未删。
fn describe_failure(target: &Path, error: &std::io::Error, noun: &str) -> String {
    let mut message = format!(
        "failed to delete {} '{}': {}",
        noun,
        target.display(),
        error
    );
    if std::fs::symlink_metadata(target).is_ok() {
        message.push_str("; the target still exists and may have been partially deleted");
    } else {
        message
            .push_str("; the target no longer exists, so the deletion completed before the error");
    }
    #[cfg(windows)]
    if error.kind() == ErrorKind::PermissionDenied {
        message.push_str("; if the target is read-only, remove the read-only attribute first");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 链接（含 Windows junction 的重解析点）不得被当成真实目录下降。
    #[test]
    fn junction_like_entries_are_classified_as_links() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.txt"), "keep\n").unwrap();
        let link = dir.path().join("link");

        if !try_link_dir(&target, &link) {
            eprintln!("directory links unavailable on this host; skipping");
            return;
        }

        let metadata = std::fs::symlink_metadata(&link).unwrap();
        assert!(
            is_link_like(&metadata),
            "a directory link must never be walked as a real directory"
        );
    }

    #[cfg(unix)]
    fn try_link_dir(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_link_dir(target: &Path, link: &Path) -> bool {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return true;
        }
        std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &link.display().to_string(),
                &target.display().to_string(),
            ])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
