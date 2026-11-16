use std::panic::AssertUnwindSafe;
use std::path::Path;
#[cfg(any(windows, target_os = "linux"))]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::os::fs_util;
use crate::os::path;

use super::attrs::is_link_like;
use super::exec::DeleteOutcome;
use super::scan::scan_tree;

/// 回收站分支：移动语义，可恢复。
///
/// 前置检查（分类、非空目录、预扫描）与永久分支完全一致；差别只在于落点是系统回收站。
/// 只读文件在这里允许通过：移动不触碰属性，而 `remove_file` 会顺手清掉只读位。
pub(super) fn move_to_trash(
    target: &Path,
    metadata: &std::fs::Metadata,
    recursive: bool,
) -> Result<DeleteOutcome, String> {
    let (is_directory, entries, bytes, truncated) = if is_link_like(metadata) {
        (false, 1usize, 0u64, false)
    } else if metadata.file_type().is_dir() {
        if !recursive {
            fs_util::ensure_empty_directory(target, "pass recursive = true to delete it")?;
            (true, 0usize, 0u64, false)
        } else {
            let scan = scan_tree(target)?;
            (true, scan.entries, scan.bytes, scan.truncated)
        }
    } else {
        (false, 1usize, metadata.len(), false)
    };

    #[cfg(windows)]
    if !target
        .ancestors()
        .last()
        .is_some_and(volume_has_recycle_bin)
    {
        return Err(format!(
            "'{}' is on a volume without a recycle bin; pass permanent = true only if the user \
             agreed to irreversible deletion",
            target.display()
        ));
    }

    #[cfg(any(windows, target_os = "linux"))]
    let deleted_after = trash_guard_time();
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        log::debug!("moving '{}' to the system recycle bin", target.display());
        trash::delete(target)
    })) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(trash_failure_message(target, &error.to_string())),
        Err(_) => {
            return Err(format!(
                "cannot move '{}' to the recycle bin: the recycle bin is unavailable in this \
                 thread; pass permanent = true only if the user agreed to irreversible deletion",
                target.display()
            ))
        }
    }

    #[cfg(any(windows, target_os = "linux"))]
    if verify_in_recycle_bin(target, deleted_after) == Some(false) {
        return Err(format!(
            "'{}' is gone but was not found in the recycle bin; it may have been deleted \
             permanently by the shell",
            target.display()
        ));
    }

    Ok(DeleteOutcome {
        is_directory,
        recursive,
        permanent: false,
        entries,
        bytes,
        truncated,
    })
}

/// 事后校验的时间下界：调用开始时刻 - 2s，用于容忍 shell 的时间戳精度。
///
/// `SystemTime` 的减法在边界上会 panic，因此用 `checked_sub`；取不到下界时返回 `None`，
/// 校验退化为「只按路径匹配」，不得因此 panic。
#[cfg(any(windows, target_os = "linux"))]
fn trash_guard_time() -> Option<i64> {
    SystemTime::now()
        .checked_sub(Duration::from_secs(2))
        .and_then(|moment| moment.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs() as i64)
}

/// 回收站失败文案：必须与磁盘现状一致，并明确「没有自动回退成永久删除」。
pub(crate) fn trash_failure_message(target: &Path, reason: &str) -> String {
    let mut message = format!(
        "failed to move '{}' to the recycle bin: {}",
        target.display(),
        reason
    );
    if std::fs::symlink_metadata(target).is_ok() {
        message.push_str("; the target still exists and may have been partially moved");
    } else {
        message.push_str("; the target no longer exists, so the move completed before the error");
    }
    message.push_str("; pass permanent = true only if the user agreed to irreversible deletion");
    message
}

/// 卷根是否具备回收站（Windows 前置筛查）。
///
/// 只做 `$Recycle.Bin` 存在性检查：UNC 共享没有本机回收站，直接判否；组策略可禁用回收站
/// 而目录仍在，因此这只是廉价筛查，最终结论以 [`verify_in_recycle_bin`] 为准。
#[cfg(windows)]
pub(crate) fn volume_has_recycle_bin(volume_root: &Path) -> bool {
    if is_unc_path(volume_root) {
        return false;
    }
    volume_root.join("$Recycle.Bin").is_dir()
}

#[cfg(windows)]
fn is_unc_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _))
    )
}

/// 事后校验：目标是否真的落进了回收站。
///
/// Windows 在回收站容量 / 配额不足时可能「永久删除并返回成功」，crate 的失败 sink 覆盖不到。
/// 返回 `None` 表示无法校验（平台不支持或 `list()` 本身失败），此时不改变本次结果。
#[cfg(any(windows, target_os = "linux"))]
fn verify_in_recycle_bin(target: &Path, deleted_after: Option<i64>) -> Option<bool> {
    let listed = std::panic::catch_unwind(AssertUnwindSafe(trash::os_limited::list));
    let items = match listed {
        Ok(Ok(items)) => items,
        Ok(Err(error)) => {
            log::debug!("cannot list the recycle bin: {}", error);
            return None;
        }
        Err(_) => {
            log::debug!("cannot list the recycle bin: COM initialization failed");
            return None;
        }
    };
    Some(items.iter().any(|item| {
        path::same_path(&item.original_path(), target)
            && deleted_after.is_none_or(|lower| item.time_deleted >= lower)
    }))
}
