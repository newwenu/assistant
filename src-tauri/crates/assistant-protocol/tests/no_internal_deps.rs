// I62 §6 G1/G2/G4：assistant-protocol 的依赖边与源码路径门禁。

use std::fs;
use std::path::{Path, PathBuf};

const FOREIGN_PREFIX: &str = "assistant_";
const ALLOWED_DEPENDENCIES: [&str; 3] = ["serde", "serde_json", "ts-rs"];
const TS_EXPORT_MARKER: &str = "#[ts(export";

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn src_root() -> PathBuf {
    manifest_dir().join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("source directory is readable");
    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest_dir())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// G1：源码零 `assistant_*` 路径（唯一例外是本 crate 自身 `assistant_protocol`）。
#[test]
fn g1_no_internal_crate_paths() {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut problems = Vec::new();
    for file in files {
        let text =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("{} 可读: {}", file.display(), e));
        for (index, line) in text.lines().enumerate() {
            if is_comment_line(line) {
                continue;
            }
            let mut cursor = 0usize;
            while let Some(offset) = line[cursor..].find(FOREIGN_PREFIX) {
                let start = cursor + offset;
                let tail = &line[start + FOREIGN_PREFIX.len()..];
                if !tail.starts_with("protocol") {
                    problems.push(format!("{}:{} {}", relative(&file), index + 1, line.trim()));
                }
                cursor = start + FOREIGN_PREFIX.len();
            }
        }
    }

    assert!(
        problems.is_empty(),
        "assistant-protocol 源码出现内部 crate 路径（依赖边未断开）: {problems:#?}"
    );
}

/// G2：`Cargo.toml` 依赖白名单（`serde` / `serde_json` / `ts-rs`）。
#[test]
fn g2_dependency_whitelist() {
    let path = manifest_dir().join("Cargo.toml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 可读: {}", path.display(), e));

    let mut problems = Vec::new();
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed.trim_matches(|c| c == '[' || c == ']').to_string();
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !section.ends_with("dependencies") {
            continue;
        }
        let Some(name) = trimmed.split(['=', '.']).next().map(str::trim) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        if !ALLOWED_DEPENDENCIES.contains(&name) {
            problems.push(format!("[{section}] {name}"));
        }
    }

    assert!(
        problems.is_empty(),
        "assistant-protocol 依赖白名单外的 crate: {problems:#?}"
    );
}

/// G4：DTO crate 不自行导出 TS 文件（导出统一由 tests/protocol_bindings.rs 负责）。
#[test]
fn g4_no_ts_export_attribute() {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut problems = Vec::new();
    for file in files {
        let text =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("{} 可读: {}", file.display(), e));
        for (index, line) in text.lines().enumerate() {
            if line.contains(TS_EXPORT_MARKER) {
                problems.push(format!("{}:{}", relative(&file), index + 1));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "assistant-protocol 源码出现 #[ts(export)]（导出应集中在 tests/protocol_bindings.rs）: {problems:#?}"
    );
}
