use std::io::ErrorKind;
use std::path::Path;

use crate::os::fs_util;

use super::attrs::{is_link_like, is_readonly, readonly_message};
use super::scan::scan_tree;
use super::trash::move_to_trash;

pub(crate) struct DeleteOutcome {
    pub(crate) is_directory: bool,
    pub(crate) recursive: bool,
    pub(crate) permanent: bool,
    pub(crate) entries: usize,
    pub(crate) bytes: u64,
    pub(crate) truncated: bool,
}

/// 同步删除，必须在 `spawn_blocking` 内调用。
///
/// 分类只用 `symlink_metadata`（不跟随链接），所有拒绝都发生在任何删除动作之前。
pub(crate) fn delete_path(
    target: &Path,
    recursive: bool,
    permanent: bool,
) -> Result<DeleteOutcome, String> {
    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(format!("'{}' does not exist", target.display()))
        }
        Err(error) => {
            return Err(format!("cannot access '{}': {}", target.display(), error));
        }
    };

    if permanent {
        delete_permanently(target, &metadata, recursive)
    } else {
        move_to_trash(target, &metadata, recursive)
    }
}

/// 永久删除分支：不可逆，沿用旧实现（`remove_file` / `remove_dir` / `remove_dir_all`）。
fn delete_permanently(
    target: &Path,
    metadata: &std::fs::Metadata,
    recursive: bool,
) -> Result<DeleteOutcome, String> {
    let file_type = metadata.file_type();

    if is_link_like(metadata) {
        remove_link(target)?;
        return Ok(DeleteOutcome {
            is_directory: false,
            recursive,
            permanent: true,
            entries: 1,
            bytes: 0,
            truncated: false,
        });
    }

    if file_type.is_dir() {
        if !recursive {
            fs_util::ensure_empty_directory(target, "pass recursive = true to delete it")?;
            std::fs::remove_dir(target).map_err(|error| {
                let mut message = describe_failure(target, &error, "directory");
                message.push_str("; if the directory is no longer empty, pass recursive = true");
                message
            })?;
            return Ok(DeleteOutcome {
                is_directory: true,
                recursive,
                permanent: true,
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
            permanent: true,
            entries: scan.entries,
            bytes: scan.bytes,
            truncated: scan.truncated,
        });
    }

    if is_readonly(metadata) {
        return Err(readonly_message(target));
    }
    let bytes = metadata.len();
    std::fs::remove_file(target).map_err(|error| describe_failure(target, &error, "file"))?;
    Ok(DeleteOutcome {
        is_directory: false,
        recursive,
        permanent: true,
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

/// 失败文案必须与磁盘现状一致：不得谎报成功，也不得谎报未删。
pub(crate) fn describe_failure(target: &Path, error: &std::io::Error, noun: &str) -> String {
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
    message
}
