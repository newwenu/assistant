//! 工具临时区（`tool-scratch`）的**真实所有者**（I77 §3.1）。
//!
//! 三项权利（建目录 / 观测 / 删除）此前散落三处且都绕过登记表（I77 §2）：
//! `agent_shell/spawn.rs` 自建目录、`storage/sample/rows.rs` 自遍历、
//! `conversation/mutate.rs` 直接 `remove_dir_all`。本模块把它们收进一个实现，
//! 应用层只经 [`ToolScratchStore`] 发起，不再自己拼路径或删目录（I77 INV3 / INV4）。
//!
//! 与 `E3`（`ToolResultStorage`）的差别：输出域按天数过期，临时区要求**会话期恒存在**，
//! 故回收只有两条显式来源——会话删除（`delete_session`）与用户清空（`clear`）；
//! **禁止**按 `mtime` 清扫，也**不做**孤儿差集（I77 §4.1 / §4.2）。

use std::path::{Path, PathBuf};

use super::storage::{dir_tree_stats, scratch_root_for, session_namespace, DeleteReport, DirStats};

/// 工具临时区所有者（I77 §3.1）。
///
/// 只做磁盘侧事实：**不读 DB**、不接受外部 `live` 集合（crate 边界，I77 INV5）。
/// 删除判据只来自「会话删除」与「用户清空」两个显式来源（I77 §4.2）。
pub trait ToolScratchStore: Send + Sync {
    /// 基根（`AppDirs::tool_scratch()`）；只读，供装配层核对。
    fn root(&self) -> &Path;

    /// 会话段路径；内部委托 `scratch_root_for`，不新增第二套拼法。
    fn session_root(&self, session_id: Option<&str>) -> PathBuf;

    /// 幂等建目录；已存在直接复用，不清空。shell 调用前唯一入口。
    fn prepare(&self, session_id: &str) -> std::io::Result<PathBuf>;

    /// 整树只读统计，口径与回收共用同一份遍历（`dir_tree_stats`）。
    fn stats(&self) -> std::io::Result<DirStats>;

    /// 会话结束：删整棵会话段；`NotFound` 静默。
    fn delete_session(&self, session_id: &str) -> DeleteReport;

    /// 用户显式清空：删基根下**全部**子项（含 `shared`、含活跃会话段），**保留基根本身**。
    fn clear(&self) -> DeleteReport;
}

impl std::fmt::Debug for dyn ToolScratchStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolScratchStore")
    }
}

/// 文件系统实现：基根由装配层注入（`AppDirs::tool_scratch()`），本类型不解析配置。
pub struct FileToolScratchStore {
    root: PathBuf,
}

impl FileToolScratchStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl ToolScratchStore for FileToolScratchStore {
    fn root(&self) -> &Path {
        &self.root
    }

    fn session_root(&self, session_id: Option<&str>) -> PathBuf {
        scratch_root_for(&self.root, session_id)
    }

    fn prepare(&self, session_id: &str) -> std::io::Result<PathBuf> {
        let dir = scratch_root_for(&self.root, Some(session_id));
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn stats(&self) -> std::io::Result<DirStats> {
        dir_tree_stats(&self.root)
    }

    fn delete_session(&self, session_id: &str) -> DeleteReport {
        // 空 id 与 `None` 同归 `shared`（`naming.rs` 的 `filter(|id| !id.is_empty())`）：
        // 那不是任何会话的段落，删它等于删无归属的共享落点，故守卫必须在所有者内部（I77 V9）。
        if session_namespace(Some(session_id)) == session_namespace(None) {
            return DeleteReport {
                skipped: 1,
                ..DeleteReport::default()
            };
        }
        let mut report = DeleteReport::default();
        remove_child(&scratch_root_for(&self.root, Some(session_id)), &mut report);
        report
    }

    fn clear(&self) -> DeleteReport {
        let mut report = DeleteReport::default();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            // 基根不存在是正常空态：`prepare` 会在下一次 shell 调用时重建（I77 INV8）。
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return report,
            Err(error) => {
                log::warn!(
                    "reclaim tool scratch root '{}' failed: {}",
                    self.root.display(),
                    error
                );
                report.failed = report.failed.saturating_add(1);
                return report;
            }
        };
        for entry in entries {
            match entry {
                Ok(entry) => remove_child(&entry.path(), &mut report),
                Err(error) => {
                    log::warn!("reclaim tool scratch entry failed: {error}");
                    report.failed = report.failed.saturating_add(1);
                }
            }
        }
        report
    }
}

/// 删一个子项：字节必须在删除**前**取，删完就取不到了（I77 INV8）。
fn remove_child(path: &Path, report: &mut DeleteReport) {
    let bytes = dir_tree_stats(path).map(|stats| stats.bytes).unwrap_or(0);
    match remove_entry(path) {
        Ok(true) => {
            report.removed = report.removed.saturating_add(1);
            report.removed_bytes = report.removed_bytes.saturating_add(bytes);
        }
        Ok(false) => report.skipped = report.skipped.saturating_add(1),
        Err(error) => {
            // 被占用的子项删不掉属正常结果：只计数、不上抛（I77 INV6）。
            log::warn!(
                "reclaim tool scratch '{}' failed: {}",
                path.display(),
                error
            );
            report.failed = report.failed.saturating_add(1);
        }
    }
}

/// 删除单个路径：真目录整棵删；链接与普通文件只删自身，**绝不进入链接目标**。
fn remove_entry(path: &Path) -> std::io::Result<bool> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if meta.file_type().is_symlink() {
        // Windows 下目录链接只能用 `remove_dir`（`remove_file` 对目录链接无效），文件链接反之。
        #[cfg(windows)]
        let removed = std::fs::remove_dir(path).or_else(|_| std::fs::remove_file(path));
        #[cfg(not(windows))]
        let removed = std::fs::remove_file(path);
        removed?;
        return Ok(true);
    }
    if meta.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FileToolScratchStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileToolScratchStore::new(dir.path());
        (dir, store)
    }

    /// 会话级目录跨调用共享：重复 `prepare` 不得清空上一次留下的产物（I73 §3.8 W6）。
    #[test]
    fn prepare_is_idempotent_and_keeps_existing_files() {
        let (_dir, store) = store();
        let first = store.prepare("s1").expect("first create");
        std::fs::write(first.join("kept.txt"), b"x").expect("write");
        let second = store.prepare("s1").expect("second create");
        assert_eq!(first, second, "同一会话段的路径必须稳定");
        assert!(
            second.join("kept.txt").exists(),
            "会话级临时区不得被下一次调用清空"
        );
    }

    #[test]
    fn session_root_matches_scratch_root_for() {
        let (_dir, store) = store();
        assert_eq!(
            store.session_root(Some("s1")),
            scratch_root_for(store.root(), Some("s1"))
        );
        assert_eq!(
            store.session_root(None),
            scratch_root_for(store.root(), None)
        );
    }

    #[test]
    fn delete_session_removes_tree_and_reports_bytes() {
        let (_dir, store) = store();
        let root = store.prepare("s1").expect("prepare");
        std::fs::write(root.join("a.bin"), vec![0u8; 32]).expect("write");
        let report = store.delete_session("s1");
        assert_eq!(report.removed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.removed_bytes >= 32);
        assert!(!root.exists(), "会话段必须整棵删除");
    }

    #[test]
    fn delete_session_missing_dir_is_skipped_not_failed() {
        let (_dir, store) = store();
        let report = store.delete_session("never");
        assert_eq!(report.removed, 0);
        assert_eq!(report.failed, 0);
        assert_eq!(report.skipped, 1);
    }

    /// `shared` 无会话归属：空 id 算出的命名空间就是 `shared`，所有者不得删除（I77 V9）。
    #[test]
    fn delete_session_on_empty_id_never_touches_shared() {
        let (_dir, store) = store();
        let shared = store.session_root(None);
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("keep.txt"), b"x").expect("write");
        let report = store.delete_session("");
        assert_eq!(report.removed, 0);
        assert_eq!(report.skipped, 1);
        assert!(shared.exists(), "shared 必须保留");
        assert!(shared.join("keep.txt").exists());
    }

    /// 清空：子项全删（含 `shared` 与活跃会话段）、字节按删除前统计、基根保留（I77 V7 / V9）。
    #[test]
    fn clear_removes_all_children_and_keeps_root() {
        let (_dir, store) = store();
        let alive = store.prepare("alive").expect("alive dir");
        let gone = store.prepare("gone").expect("gone dir");
        std::fs::write(alive.join("a.bin"), vec![0u8; 64]).expect("write");
        std::fs::write(gone.join("b.bin"), vec![0u8; 16]).expect("write");
        let shared = store.session_root(None);
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("c.bin"), vec![0u8; 8]).expect("write");

        let report = store.clear();
        assert_eq!(report.removed, 3, "三个命名空间都必须被清空");
        assert_eq!(report.failed, 0);
        assert!(report.removed_bytes >= 88, "字节必须在删除前统计");
        assert!(store.root().exists(), "基根必须保留");
        assert!(!alive.exists());
        assert!(!gone.exists());
        assert!(!shared.exists(), "shared 随清空一并删除，不再有永续特例");
        assert_eq!(
            std::fs::read_dir(store.root()).expect("read").count(),
            0,
            "基根下不得残留子项"
        );
    }

    /// 基根不存在是正常空态：清空零副作用，不报失败（`prepare` 会自愈）。
    #[test]
    fn clear_on_missing_root_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileToolScratchStore::new(dir.path().join("absent"));
        let report = store.clear();
        assert_eq!(report.removed, 0);
        assert_eq!(report.failed, 0);
        assert_eq!(report.skipped, 0);
    }

    /// 清空后 `prepare` 重建会话段（自愈，I77 V8）。
    #[test]
    fn prepare_recreates_after_clear() {
        let (_dir, store) = store();
        store.prepare("s1").expect("prepare");
        store.clear();
        let rebuilt = store.prepare("s1").expect("rebuild");
        assert!(rebuilt.exists(), "清空后下一次调用必须自愈");
    }
}
