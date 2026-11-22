//! 测试夹具：创建链接。
//!
//! 各测试文件曾各写一份，Windows 侧漂成两种策略——只调 `symlink_dir` 的版本在未开
//! Developer Mode 的机器上必然失败，用例随即静默跳过，覆盖凭空消失。这里统一策略：
//! 目录链接优先 `symlink_dir`，失败回落 `mklink /J`（junction 不需要特权）；
//! 文件链接没有等价替代，失败即返回 false，由调用方显式跳过。
//!
//! 本文件同时被 `os::test_support`（`src/os/shared/test_support.rs`）以 `#[path]`
//! 引入，并随 `test-support` feature 公开给其它 package 的集成测试：集成测试看不到
//! lib 的 `#[cfg(test)]` 项，内联测试也看不到集成测试的模块，共用一份才能避免再次漂移。
//!
//! 参数顺序与 `std::os::unix::fs::symlink` 一致：目标在前、链接路径在后。

#![allow(dead_code)]

use std::path::Path;

#[cfg(unix)]
pub fn file_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
pub fn file_link(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(not(any(unix, windows)))]
pub fn file_link(_target: &Path, _link: &Path) -> bool {
    false
}

#[cfg(unix)]
pub fn dir_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
pub fn dir_link(target: &Path, link: &Path) -> bool {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return true;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
pub fn dir_link(_target: &Path, _link: &Path) -> bool {
    false
}
