use std::path::Path;

use crate::r#trait::ApprovalPreview;

use super::attrs::is_link_like;

/// 预扫描条目上限：达到即停止并标记 `truncated`，但**不**因此拒绝删除。
pub(crate) const DELETE_SCAN_LIMIT: usize = 100_000;

pub(crate) struct ScanOutcome {
    pub(crate) entries: usize,
    pub(crate) bytes: u64,
    pub(crate) truncated: bool,
}

/// 递归预扫描：显式栈 + `read_dir`，不下降链接。
///
/// 只服务两件事：非空目录判定与量级文案。**不承担安全职责**——边界已由 `resolve` 一次性
/// 确定，子树也无法借链接逃逸（链接不下降）。`preview_for_approval` 与 `execute` 共用
/// 这一份实现，避免「审批说 3 项、实际删 300 项」。
pub(crate) fn scan_tree(root: &Path) -> Result<ScanOutcome, String> {
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

/// 预演：只回填结构化量级，`summary` 留给诊断（I72 §5.3）。
///
/// 量级来自 `scan_tree`（与 `execute` 同源），因此 `estimated = false`；
/// 目标已消失（`symlink_metadata` 失败）时返回 `None`，由调用方回落到无预演。
pub(crate) fn describe_preview(target: &Path) -> Option<ApprovalPreview> {
    let metadata = std::fs::symlink_metadata(target).ok()?;
    let file_type = metadata.file_type();

    if is_link_like(&metadata) {
        return Some(ApprovalPreview {
            summary: String::new(),
            truncated: false,
            estimated: false,
            entries: Some(1),
            bytes: None,
            target_kind: Some("link".to_string()),
            exists: Some(true),
            symlink: Some(true),
            verb_code: None,
            shell: None,
        });
    }
    if !file_type.is_dir() {
        return Some(ApprovalPreview {
            summary: String::new(),
            truncated: false,
            estimated: false,
            entries: Some(1),
            bytes: Some(metadata.len()),
            target_kind: Some("file".to_string()),
            exists: Some(true),
            symlink: Some(false),
            verb_code: None,
            shell: None,
        });
    }

    let (entries, bytes, truncated, summary) = match scan_tree(target) {
        Ok(scan) => (
            Some(scan.entries as u64),
            Some(scan.bytes),
            scan.truncated,
            String::new(),
        ),
        Err(error) => (None, None, false, error),
    };
    Some(ApprovalPreview {
        summary,
        truncated,
        estimated: false,
        entries,
        bytes,
        target_kind: Some("directory".to_string()),
        exists: Some(true),
        symlink: Some(false),
        verb_code: None,
        shell: None,
    })
}
