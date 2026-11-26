// 参考: docs/store/I75-startup-layering-spec.md §5 — `assembly_contract` / `startup_contract` 共用
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const TAURI_CONF_FILE: &str = "tauri.conf.json";

pub fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

pub fn src_root() -> PathBuf {
    manifest_dir().join("src")
}

pub fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
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

pub fn read(relative: &str) -> String {
    let path = src_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {}", relative, e))
}

pub fn strip_rust_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev = '\0';
                    for c in chars.by_ref() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                    out.push(' ');
                    continue;
                }
                _ => {}
            }
        }
        out.push(ch);
    }
    out
}

pub fn declared_type_names(dir: &Path) -> BTreeSet<String> {
    let mut files = Vec::new();
    if dir.is_dir() {
        collect_rs_files(dir, &mut files);
    }
    let mut names = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        for line in text.lines() {
            let trimmed = line.trim();
            for prefix in ["pub struct ", "pub enum ", "pub type "] {
                let Some(rest) = trimmed.strip_prefix(prefix) else {
                    continue;
                };
                let name = rest
                    .split(|c: char| c.is_whitespace() || matches!(c, '{' | '(' | '<' | ';'))
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// 窗口标签的唯一来源：`assistant_config::window_spec::WINDOW_SPECS`。
///
/// `tauri.conf.json` 的 `app.windows` 已置空（G4），不再作为基准。
pub fn window_labels() -> Vec<String> {
    assistant_config::window_spec::WINDOW_SPECS
        .iter()
        .map(|spec| spec.label.to_string())
        .collect()
}

/// `tauri.conf.json` 的 `app.windows` 数组：G4 断言其为空。
pub fn tauri_conf_windows() -> Vec<serde_json::Value> {
    let path = manifest_dir().join(TAURI_CONF_FILE);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 不可读: {}", path.display(), e));
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("tauri.conf.json 是合法 JSON");
    value
        .get("app")
        .and_then(|app| app.get("windows"))
        .and_then(|windows| windows.as_array())
        .cloned()
        .unwrap_or_default()
}
