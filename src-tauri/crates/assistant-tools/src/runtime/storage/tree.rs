use std::path::Path;
use std::time::UNIX_EPOCH;

/// 受管目录的观测快照（I61 §3.10）：条目与清理共用同一口径。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub count: u64,
    /// 目录内文件 `mtime` 最早 / 最晚时刻（秒）；无文件时两端为 `None`，不是 `0`。
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 与 [`cleanup_expired_results`](super::retention::cleanup_expired_results) 同一套遍历规则
/// （递归 / 跳符号链接 / 跳非常规文件），但不按年龄过滤：统计的是整个条目当前占用。
pub fn result_stats(root: &Path) -> std::io::Result<DirStats> {
    dir_tree_stats(root)
}

/// 整棵子树的只读统计：crate 内所有「受管目录条目」共用同一份遍历实现。
///
/// 任何调用方自己写 `read_dir` 循环都会让统计口径与回收口径漂移（I61 G4），
/// 因此这里只留一个入口，外部只能经由它拿数字。
pub(crate) fn dir_tree_stats(root: &Path) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    let mut first_error: Option<std::io::Error> = None;
    for_each_file(
        root,
        &mut |path, meta| {
            if !is_companion_file(path) {
                accumulate_stats(&mut stats, meta);
            }
        },
        &mut |path, error| {
            if first_error.is_none() {
                first_error = Some(traversal_error(path, error));
            }
        },
    );
    match first_error {
        Some(error) => Err(error),
        None => Ok(stats),
    }
}

/// 按后缀批量删除的计数：成功条数 / 成功字节 / 失败条数 / 判据排除条数 / 目录读失败数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
    /// 遍历时读目录失败（权限 / 文件正在替换）：一个候选都没处理成功时据此判 `failed`。
    pub read_failures: u64,
}

/// 受管目录的「按后缀批量删除」：与 [`dir_tree_stats`] 共用 [`for_each_file`]。
///
/// 只删 `suffix` 命中的常规文件（大小写不敏感）；符号链接 / 非常规文件 / 子目录不参与，
/// 因此「统计多少」与「能删多少」不会各说一套（I61 G4）。目录不存在是正常空态，
/// 单条删除失败只计数、不中断。
pub(crate) fn purge_tree_files(root: &Path, suffix: &str) -> PurgeReport {
    let suffix = suffix.to_ascii_lowercase();
    let mut report = PurgeReport::default();
    for_each_file(
        root,
        &mut |path, meta| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                report.skipped = report.skipped.saturating_add(1);
                return;
            };
            if !name.to_ascii_lowercase().ends_with(&suffix) {
                return;
            }
            match std::fs::remove_file(path) {
                Ok(()) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(meta.len());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.skipped = report.skipped.saturating_add(1);
                }
                Err(_) => report.failed = report.failed.saturating_add(1),
            }
        },
        &mut |_path, _error| {
            report.read_failures = report.read_failures.saturating_add(1);
        },
    );
    report
}

/// 受管目录的统一遍历：递归、跳符号链接、跳非常规文件。
///
/// 清理与统计必须共用本函数：任何一处自己写 `read_dir` 循环，
/// 「统计多少」与「能删多少」就会漂移（I61 G4）。
///
/// 目录不存在是正常空态（按需创建），静默返回；其余失败交给 `on_error`，
/// 由调用方决定是记日志还是向上返回——本函数本身永不中断遍历。
pub(super) fn for_each_file(
    dir: &Path,
    on_file: &mut dyn FnMut(&Path, &std::fs::Metadata),
    on_error: &mut dyn FnMut(&Path, &std::io::Error),
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            on_error(dir, &error);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                on_error(dir, &error);
                continue;
            }
        };
        let path = entry.path();
        // `symlink_metadata` 不跟随链接：符号链接按「非常规文件」跳过，绝不删目标。
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                on_error(&path, &error);
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            for_each_file(&path, on_file, on_error);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        on_file(&path, &meta);
    }
}

/// 伴生文件（SQLite 边车 / 部分写入残留）不计入统计；清理仍会遍历到它们（§3.9）。
const COMPANION_SUFFIXES: &[&str] = &["-journal", "-wal", "-shm", ".tmp", ".lock"];

fn is_companion_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    COMPANION_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn accumulate_stats(stats: &mut DirStats, meta: &std::fs::Metadata) {
    stats.bytes = stats.bytes.saturating_add(meta.len());
    stats.count = stats.count.saturating_add(1);
    if let Ok(modified) = meta.modified() {
        if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
            let seconds = i64::try_from(since_epoch.as_secs()).unwrap_or(i64::MAX);
            stats.oldest_at = Some(stats.oldest_at.map_or(seconds, |cur| cur.min(seconds)));
            stats.newest_at = Some(stats.newest_at.map_or(seconds, |cur| cur.max(seconds)));
        }
    }
}

fn traversal_error(path: &Path, error: &std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("scan '{}' failed: {}", path.display(), error),
    )
}
