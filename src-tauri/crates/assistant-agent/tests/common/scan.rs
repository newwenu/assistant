//! 源码扫描门禁：`kernel_ports` 与 `kernel_values` 共用同一套目录遍历与禁用词断言。
//!
//! 这些用例断言的是「内核源码里没有某类调用/分支」，重复实现会让新增禁用词
//! 需要改多处；集中到此处后，新增守卫只需在用例里加词表。

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// 递归收集目录下的 `.rs` 文件。
pub fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read kernel dir");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// 收集 `src/kernel` 下全部源文件，并断言目录非空（防止路径失效后守卫空跑）。
pub fn kernel_sources() -> Vec<PathBuf> {
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kernel");
    let mut sources = Vec::new();
    collect_rs_files(&kernel_dir, &mut sources);
    assert!(!sources.is_empty(), "kernel sources not found");
    sources
}

/// 断言给定源文件中不出现任何禁用词；`what` 用于说明被禁对象的类别。
pub fn assert_no_forbidden(sources: &[PathBuf], forbidden: &[&str], what: &str) {
    assert_absent(sources, forbidden, |path, needle| {
        format!("{} contains forbidden {} {}", path.display(), what, needle)
    });
}

/// 断言给定源文件中不出现任何禁用词，失败信息由调用点自定义。
///
/// 用于「词表含义各不相同」的场景，例如未接线字段清单需要提示同步更新文档。
pub fn assert_absent(
    sources: &[PathBuf],
    forbidden: &[&str],
    message: impl Fn(&Path, &str) -> String,
) {
    for path in sources {
        let text = std::fs::read_to_string(path).expect("read kernel source");
        for needle in forbidden {
            assert!(!text.contains(needle), "{}", message(path, needle));
        }
    }
}
