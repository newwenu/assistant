// 参考: docs/store/I75-startup-layering-spec.md §5 / §10.1 — 从 `setup.rs` 迁出的 DB 句柄

use std::sync::{Arc, Mutex};

pub struct DbState(pub Arc<Mutex<rusqlite::Connection>>, pub std::path::PathBuf);

/// 句柄可克隆：`Arc` 让观测能在 `spawn_blocking` 里拿着 `'static` 的 owner 句柄，
/// 路径同样随句柄走（G1：管理器不自拼路径）。
impl Clone for DbState {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), self.1.clone())
    }
}

impl DbState {
    /// 路径由本状态自持（G1：管理器不自拼路径），只读 `metadata`。
    ///
    /// 文件不存在返回全零（`missing` 语义），其余 `metadata` 失败返回 `Err`
    /// 由管理器降级为 `unavailable`（文件正被替换 / 迁移，§7.1）。
    pub fn db_file_stats(&self) -> std::io::Result<crate::app_dirs::fs_stats::DirStats> {
        crate::app_dirs::fs_stats::file_stats(&self.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(path: std::path::PathBuf) -> DbState {
        DbState(
            Arc::new(Mutex::new(
                rusqlite::Connection::open_in_memory().expect("内存库应可创建"),
            )),
            path,
        )
    }

    #[test]
    fn missing_db_file_reports_zero_and_creates_nothing() {
        let dir = tempfile::tempdir().expect("临时目录应可创建");
        let path = dir.path().join("assistant.db");
        let stats = state(path.clone())
            .db_file_stats()
            .expect("文件缺失应为空态而不是错误");

        assert_eq!(stats.bytes, 0, "缺失按 missing 计 0，不是降级");
        assert_eq!(stats.count, 0);
        assert_eq!(stats.oldest_at, None);
        assert_eq!(stats.newest_at, None);
        assert!(!path.exists(), "观测只 metadata，绝不创建 / 打开库");
    }

    #[test]
    fn existing_db_file_is_measured_by_metadata_only() {
        let dir = tempfile::tempdir().expect("临时目录应可创建");
        let path = dir.path().join("assistant.db");
        std::fs::write(&path, b"not-a-db").expect("写夹具应成功");

        let stats = state(path.clone()).db_file_stats().expect("stat 应成功");
        assert_eq!(stats.bytes, 8, "按文件长度计");
        assert_eq!(stats.count, 1);
        assert!(stats.oldest_at.is_some(), "有文件时必须给出时刻");
    }
}
