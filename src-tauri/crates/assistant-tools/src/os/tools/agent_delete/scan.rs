use std::path::Path;

use crate::os::output::human_bytes;
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

pub(crate) fn describe_preview(
    target: &Path,
    recursive: bool,
    permanent: bool,
) -> Option<ApprovalPreview> {
    let metadata = std::fs::symlink_metadata(target).ok()?;
    let file_type = metadata.file_type();

    if is_link_like(&metadata) {
        let summary = if permanent {
            format!(
                "delete the link at {} (its target is not touched)",
                target.display()
            )
        } else {
            format!(
                "move the link at {} to the system recycle bin (its target is not touched)",
                target.display()
            )
        };
        return Some(ApprovalPreview {
            summary,
            truncated: false,
            estimated: true,
        });
    }
    if !file_type.is_dir() {
        let size = human_bytes(metadata.len() as usize);
        let summary = if permanent {
            format!(
                "permanently delete the file at {} ({})",
                target.display(),
                size
            )
        } else {
            format!(
                "move the file at {} ({}) to the system recycle bin",
                target.display(),
                size
            )
        };
        return Some(ApprovalPreview {
            summary,
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
    let summary = match (permanent, recursive) {
        (true, true) => format!(
            "permanently delete the directory at {} ({}, {})",
            target.display(),
            entries,
            bytes
        ),
        (true, false) => format!(
            "delete the directory at {} ({}, {}); it is only deleted if it is empty",
            target.display(),
            entries,
            bytes
        ),
        (false, true) => format!(
            "move the directory at {} ({}, {}) to the system recycle bin",
            target.display(),
            entries,
            bytes
        ),
        (false, false) => format!(
            "move the directory at {} ({}, {}) to the system recycle bin; it is only moved if it is empty",
            target.display(),
            entries,
            bytes
        ),
    };
    Some(ApprovalPreview {
        summary,
        truncated: scan.truncated,
        estimated: true,
    })
}
