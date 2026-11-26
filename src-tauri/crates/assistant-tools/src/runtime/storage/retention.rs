use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::tree::for_each_file;

/// `E3` 的回收候选：路径 + 收集时的字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredCandidate {
    pub path: PathBuf,
    pub bytes: u64,
}

/// 候选删除结果：成功条数 / 成功字节 / 失败条数 / 已不存在条数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeleteReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    /// 执行时文件已不存在（并发清理或用户手删）：不算失败，也不计入删除量（§5.1 第 3 条）。
    pub skipped: u64,
}

/// 启动清理结果：只用于日志与测试断言，不参与任何业务判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: usize,
    pub failed: usize,
}

/// 按 `mtime` 年龄清理落盘目录（I46 compact §2.9 D9，门禁 10）。
///
/// 约束：只删常规文件、不跟随符号链接、失败只计数不抛出、`retention_days == 0`
/// 完全禁用。目录本身不修剪（OQ6 定稿：只删文件、保留空目录）。
/// 旧布局（根目录下的扁平 `<call_id>.txt`）同样按年龄清理，不做迁移。
pub fn cleanup_expired_results(root: &Path, retention_days: u32) -> CleanupReport {
    let mut report = CleanupReport::default();
    if retention_days == 0 {
        return report;
    }
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return report;
    };
    sweep_directory(root, cutoff, &mut report);
    report
}

/// `E3` 的候选收集：与 [`cleanup_expired_results`] 同一判据、同一遍历，额外带字节。
///
/// `retention_days == 0` 时返回空集（策略禁用，不产生任何候选）；字节取收集时刻的
/// `metadata.len()`，执行时不再重算（删完就取不到）。
pub fn collect_expired(root: &Path, retention_days: u32) -> Vec<ExpiredCandidate> {
    if retention_days == 0 {
        return Vec::new();
    }
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for_each_file(
        root,
        &mut |path, meta| {
            if is_expired(meta, cutoff) {
                candidates.push(ExpiredCandidate {
                    path: path.to_path_buf(),
                    bytes: meta.len(),
                });
            }
        },
        &mut |_path, _error| {},
    );
    candidates
}

/// `E3` 的候选删除：逐条删除并累加收集时的字节。
///
/// 「文件已不存在」计入 `skipped`（并发启动清理 / 用户手删），不算失败；
/// 其余错误只计数不抛出，单条失败不影响其余候选（§5.1 第 2、3 条）。
pub fn delete_candidates(candidates: &[ExpiredCandidate]) -> DeleteReport {
    let mut report = DeleteReport::default();
    for candidate in candidates {
        match std::fs::remove_file(&candidate.path) {
            Ok(()) => {
                report.removed = report.removed.saturating_add(1);
                report.removed_bytes = report.removed_bytes.saturating_add(candidate.bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.skipped = report.skipped.saturating_add(1);
            }
            Err(error) => {
                log::warn!("reclaim tool results: remove candidate failed: {}", error);
                report.failed = report.failed.saturating_add(1);
            }
        }
    }
    report
}

/// 过期判据（唯一来源）：`mtime` 严格早于截止时刻。
///
/// `mtime` 取不到或落在未来都判「未过期」（fail-closed，§5.5）：宁可少删，
/// 不因时钟回拨 / 文件系统精度问题误删刚写入的结果。
fn is_expired(meta: &std::fs::Metadata, cutoff: SystemTime) -> bool {
    meta.modified().map(|mtime| mtime < cutoff).unwrap_or(false)
}

fn retention_cutoff(retention_days: u32) -> Option<SystemTime> {
    let span = Duration::from_secs(u64::from(retention_days).saturating_mul(86_400));
    SystemTime::now().checked_sub(span)
}

fn sweep_directory(dir: &Path, cutoff: SystemTime, report: &mut CleanupReport) {
    // 计数用 `Cell`：遍历的两个回调各自捕获同一份计数，不产生双重可变借用。
    let removed = std::cell::Cell::new(0usize);
    let failed = std::cell::Cell::new(0usize);
    for_each_file(
        dir,
        &mut |path, meta| {
            let expired = is_expired(meta, cutoff);
            if !expired {
                return;
            }
            match std::fs::remove_file(path) {
                Ok(()) => removed.set(removed.get() + 1),
                Err(error) => {
                    log::warn!(
                        "cleanup tool results: remove '{}' failed: {}",
                        path.display(),
                        error
                    );
                    failed.set(failed.get() + 1);
                }
            }
        },
        &mut |path, error| {
            log::warn!(
                "cleanup tool results: scan '{}' failed: {}",
                path.display(),
                error
            );
            failed.set(failed.get() + 1);
        },
    );
    report.removed += removed.get();
    report.failed += failed.get();
}
