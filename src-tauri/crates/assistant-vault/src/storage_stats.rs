//! 安全域受管目录的观测快照（I61 §3.10）：条目 `E7` / `E8` / `E11` 共用同一口径。
//!
//! 只读、幂等、不持锁：管理器在 `spawn_blocking` 里调用，超时后线程仍会跑完，
//! 因此这里不碰 `SecurityDbState::session` 锁，也不做任何写入。

use std::path::Path;
use std::time::UNIX_EPOCH;

/// 观测快照：`bytes` / `count` 与 `oldest_at` / `newest_at` 一次遍历同时给出。
///
/// 无文件时 `oldest_at` / `newest_at` 为 `None`，不是 `0`（§3.9）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub count: u64,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 单文件观测：只 `metadata`，不开库、不读取内容（`E7`）。
///
/// 文件不存在返回全零（`missing` 语义）；其余失败返回错误类别由管理器降级为
/// `unavailable`（文件正被替换 / 迁移，§7.1）。符号链接与非常规文件不计入。
pub fn file_stats(path: &Path) -> std::io::Result<DirStats> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DirStats::default());
        }
        Err(error) => return Err(stat_error(path, error)),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Ok(DirStats::default());
    }
    let mut stats = DirStats::default();
    accumulate(&mut stats, &meta);
    Ok(stats)
}

/// 目录观测（`E8` / `E11`）：只统计 `accept` 认可的文件名，口径与各自的清理函数一致。
///
/// 目录不存在是正常空态，返回全零而不是错误；符号链接与非常规文件跳过、不降级。
pub fn dir_stats<F>(dir: &Path, accept: F) -> std::io::Result<DirStats>
where
    F: Fn(&Path) -> bool,
{
    let mut stats = DirStats::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(stats),
        Err(error) => return Err(read_dir_error(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| bad_entry_error(dir, error))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|error| stat_error(&path, error))?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        if !accept(&path) {
            continue;
        }
        accumulate(&mut stats, &meta);
    }
    Ok(stats)
}

/// 整棵子树的观测（`E9` / `E10`）：仓库与恢复副本都是多层目录。
///
/// 遍历规则与受管目录登记表的 `RecursiveTree` 覆盖一致：递归、跳符号链接、
/// 跳非常规文件；目录不存在是正常空态。
pub fn tree_stats(dir: &Path) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    walk_tree(dir, &mut stats)?;
    Ok(stats)
}

fn walk_tree(dir: &Path, stats: &mut DirStats) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(read_dir_error(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| bad_entry_error(dir, error))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|error| stat_error(&path, error))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk_tree(&path, stats)?;
            continue;
        }
        if !meta.is_file() || is_companion_file(&path) {
            continue;
        }
        accumulate(stats, &meta);
    }
    Ok(())
}

/// 伴生文件（SQLite 边车 / 部分写入残留）不计入条目数字（§3.9 包含范围）。
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

fn accumulate(stats: &mut DirStats, meta: &std::fs::Metadata) {
    stats.bytes = stats.bytes.saturating_add(meta.len());
    stats.count = stats.count.saturating_add(1);
    if let Some(seconds) = mtime_seconds(meta) {
        stats.oldest_at = Some(stats.oldest_at.map_or(seconds, |cur| cur.min(seconds)));
        stats.newest_at = Some(stats.newest_at.map_or(seconds, |cur| cur.max(seconds)));
    }
}

fn mtime_seconds(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|since_epoch| i64::try_from(since_epoch.as_secs()).ok())
}

fn stat_error(path: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("stat '{}' failed: {}", path.display(), error),
    )
}

fn read_dir_error(dir: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("read dir '{}' failed: {}", dir.display(), error),
    )
}

fn bad_entry_error(dir: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("bad entry in '{}': {}", dir.display(), error),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("assistant-vault-stats-{}-{}", name, nanos))
    }

    #[test]
    fn missing_path_reports_zero_without_error() {
        let path = unique_dir("missing");
        assert_eq!(
            file_stats(&path).expect("缺失文件应为空态"),
            DirStats::default()
        );
        assert_eq!(
            dir_stats(&path, |_| true).expect("缺失目录应为空态"),
            DirStats::default()
        );
        assert_eq!(
            tree_stats(&path).expect("缺失子树应为空态"),
            DirStats::default()
        );
    }

    #[test]
    fn dir_stats_counts_only_accepted_regular_files() {
        let dir = unique_dir("dir");
        std::fs::create_dir_all(&dir).expect("临时目录应可创建");
        std::fs::write(dir.join("audit-20250101.jsonl"), b"1234").expect("写入应成功");
        std::fs::write(dir.join("other.txt"), b"12").expect("写入应成功");

        let stats = dir_stats(&dir, |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".jsonl"))
        })
        .expect("扫描应成功");
        assert_eq!(stats.count, 1);
        assert_eq!(stats.bytes, 4);
        assert!(stats.oldest_at.is_some());
        assert_eq!(stats.oldest_at, stats.newest_at);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tree_stats_recurses_and_skips_non_regular_files() {
        let dir = unique_dir("tree");
        std::fs::create_dir_all(dir.join("nested")).expect("临时目录应可创建");
        std::fs::write(dir.join("nested").join("a.bin"), b"123").expect("写入应成功");
        std::fs::write(dir.join("b.bin"), b"12").expect("写入应成功");

        let stats = tree_stats(&dir).expect("扫描应成功");
        assert_eq!(stats.count, 2);
        assert_eq!(stats.bytes, 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_stats_reads_length_and_mtime() {
        let path = unique_dir("file").join("security.db");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("临时目录应可创建");
        }
        std::fs::write(&path, b"abcdef").expect("写入应成功");
        let stats = file_stats(&path).expect("stat 应成功");
        assert_eq!(stats.bytes, 6);
        assert_eq!(stats.count, 1);
        assert!(stats.oldest_at.is_some());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
