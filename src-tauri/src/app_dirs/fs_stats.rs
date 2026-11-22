//! 应用自有目录的观测工具（I61 §3.10）：`E5` / `E6` 及后续 app 自有条目共用。
//!
//! 只读、幂等、不持锁：管理器在 `spawn_blocking` 里调用，超时后线程仍会跑完。
//! 遍历规则：不跟随符号链接、跳过非常规文件、目录自身不计；伴生文件
//! （`-journal` / `-wal` / `-shm` / `.tmp` / `.lock`）按普通文件计入（见 [`tree_stats`]）。
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
/// 伴生文件（`-journal` / `-wal` / `-shm` / `.tmp` / `.lock`）与普通文件同口径计入
/// `bytes` / `count`：它们确实占盘，对账扫描（`inventory::walk`）也把它们计入
/// `scanBytes`；两侧同口径，「已登记」才能覆盖全部已观测字节，不留假余量。
pub fn tree_stats(dir: &Path) -> std::io::Result<DirStats> {
    tree_stats_with(dir, &mut |_, _| {})
}

/// 与 [`tree_stats`] 同一遍历，但把每个**已计入**文件回调给调用方。
///
/// 用于「整棵树 + 其中某个子集」要同时取数的场景（`E14` 的整棵树与缓存白名单）：
/// 一次遍历给出两个量，避免同一棵树走两遍把元数据访问翻倍（采样预算见
/// `sample::timeout`）。回调在跳符号链接 / 非常规文件之后触发，口径与 `tree_stats`
/// 恒等（伴生文件同样计入）。
pub fn tree_stats_with(
    dir: &Path,
    on_file: &mut dyn FnMut(&Path, &std::fs::Metadata),
) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    walk_tree(dir, &mut stats, on_file)?;
    Ok(stats)
}

fn walk_tree(
    dir: &Path,
    stats: &mut DirStats,
    on_file: &mut dyn FnMut(&Path, &std::fs::Metadata),
) -> std::io::Result<()> {
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
            walk_tree(&path, stats, on_file)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        on_file(&path, &meta);
        accumulate(stats, &meta);
    }
    Ok(())
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
    fn tree_stats_with_sees_exactly_the_counted_files() {
        let dir = unique_dir("tree-with");
        std::fs::create_dir_all(dir.join("nested")).expect("临时目录应可创建");
        std::fs::write(dir.join("nested").join("a.bin"), b"123").expect("写入应成功");
        std::fs::write(dir.join("b.bin"), b"12").expect("写入应成功");
        std::fs::write(dir.join("c.bin.tmp"), b"ignored").expect("写入应成功");

        let mut seen: Vec<std::path::PathBuf> = Vec::new();
        let stats = tree_stats_with(&dir, &mut |path, _| seen.push(path.to_path_buf()))
            .expect("扫描应成功");
        assert_eq!(stats.count, 3, "回调覆盖全部计入的文件（伴生文件同样计入）");
        assert_eq!(stats.bytes, 12);
        assert_eq!(seen.len(), 3);
        assert_eq!(stats, tree_stats(&dir).expect("两次口径应一致"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tree_stats_counts_companion_files() {
        let dir = unique_dir("companion");
        std::fs::create_dir_all(&dir).expect("临时目录应可创建");
        for name in [
            "a.bin-journal",
            "b.bin-wal",
            "c.bin-shm",
            "d.bin.tmp",
            "e.bin.lock",
        ] {
            std::fs::write(dir.join(name), b"12").expect("写入应成功");
        }
        std::fs::write(dir.join("f.bin"), b"123").expect("写入应成功");

        let stats = tree_stats(&dir).expect("扫描应成功");
        assert_eq!(stats.count, 6, "伴生文件与普通文件同口径计入");
        assert_eq!(stats.bytes, 13, "伴生文件确实占盘，不能漏计");

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
