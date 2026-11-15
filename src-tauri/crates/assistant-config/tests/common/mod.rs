#![allow(dead_code)]

use std::path::PathBuf;

use tempfile::TempDir;

/// 返回仍存活的临时目录 guard 与配置路径。
///
/// 只返回路径会让 `TempDir` 在语句结束时就析构并删除目录，之后的 `save_to`
/// 重建父目录便再无人回收，导致每跑一次配置测试都向 `%TEMP%` 泄漏目录。
pub fn conf_path(name: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    (dir, path)
}
