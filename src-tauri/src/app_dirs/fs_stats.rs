//! 应用自有目录的观测工具（I61 §3.10）：`E5` / `E6` 及后续 app 自有条目共用。
//!
//! 只读、幂等、不持锁：管理器在 `spawn_blocking` 里调用，超时后线程仍会跑完。
//! 遍历规则与 §3.9 一致：不跟随符号链接、跳过非常规文件、目录自身不计。
//!
//! 为什么不在各 crate 之间共享一份实现：`assistant-tools` / 密码库域 / `assistant-crash`
//! 是互不依赖的域 crate，为一个 4 字段结构互相引入依赖会把无关模块绑在一起。
//! 观测形状只在**同一 crate 内**共享（如 tools 的 `E3`/`E4`）。

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

/// 单文件观测：只 `metadata`，不开库、不读取内容（`E6` / `E7` 形状）。
///
/// 文件不存在返回全零（`missing` 语义）；其余失败返回 `Err` 由管理器降级为
/// `unavailable`（文件正被替换 / 迁移，§7.1）。符号链接与非常规文件不计入。
pub fn file_stats(path: &Path) -> std::io::Result<DirStats> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DirStats::default());
        }
        Err(error) => return Err(traversal_error(path, error)),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Ok(DirStats::default());
    }
    let mut stats = DirStats::default();
    accumulate(&mut stats, &meta);
    Ok(stats)
}

/// 整棵子树的观测（`E5` 等递归条目）：递归、跳符号链接、跳非常规文件。
///
/// 目录不存在是正常空态（按需创建），返回全零而不是错误。
/// 伴生文件（`.tmp` 中间态等）不计入 `bytes` / `count`（§3.9 包含范围）。
pub fn tree_stats(dir: &Path) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    walk_tree(dir, &mut stats)?;
    Ok(stats)
}

fn walk_tree(dir: &Path, stats: &mut DirStats) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(traversal_error(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| traversal_error(dir, error))?;
        let path = entry.path();
        let meta =
            std::fs::symlink_metadata(&path).map_err(|error| traversal_error(&path, error))?;
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

/// 伴生文件（SQLite 边车 / 部分写入残留）：不计入条目数字，由 owner 在回收时一并清理（§3.9）。
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

fn traversal_error(path: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("scan '{}' failed: {}", path.display(), error),
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
        std::env::temp_dir().join(format!("assistant-app-stats-{}-{}", name, nanos))
    }

    #[test]
    fn missing_path_reports_zero_without_error() {
        let path = unique_dir("missing");
        assert_eq!(
            file_stats(&path).expect("缺失文件应为空态"),
            DirStats::default()
        );
        assert_eq!(
            tree_stats(&path).expect("缺失子树应为空态"),
            DirStats::default()
        );
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
        assert!(stats.oldest_at.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_stats_reads_length_and_mtime() {
        let path = unique_dir("file").join("assistant.db");
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
