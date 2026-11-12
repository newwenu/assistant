use std::fs;
use std::path::{Path, PathBuf};

use assistant_crypto::vault_crypto::types::{DbKey, Dek, Kek, MetaKey};

fn src_tauri_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

const SCAN_DIRS: [&str; 4] = [
    "crates/assistant-vault/src",
    "crates/assistant-crypto/src",
    "crates/assistant-security/src",
    "src/cmd/vault",
];

const LOG_MACROS: [&str; 5] = ["info!", "warn!", "error!", "debug!", "trace!"];

/// 只允许「不携带密钥材料」的日志：出现这些取明文/取密钥的调用即视为泄露风险。
const FORBIDDEN_TOKENS: [&str; 6] = [
    ".expose(",
    ".expose_str(",
    ".as_bytes(",
    ".to_bytes(",
    ".into_inner(",
    ".into_zeroizing(",
];

fn log_macro_bodies(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bodies = Vec::new();
    for macro_name in LOG_MACROS {
        let mut search_from = 0;
        while let Some(offset) = text[search_from..].find(macro_name) {
            let start = search_from + offset;
            search_from = start + macro_name.len();
            if start > 0 {
                let prev = bytes[start - 1];
                let is_boundary = !(prev.is_ascii_alphanumeric() || prev == b'_');
                if !is_boundary {
                    continue;
                }
            }
            let open = match text[search_from..].find('(') {
                Some(p) => search_from + p,
                None => continue,
            };
            if open != search_from {
                continue;
            }
            let mut depth = 0usize;
            let mut end = None;
            for (i, ch) in text[open..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(open + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end {
                bodies.push(text[start..end].to_string());
                search_from = end;
            }
        }
    }
    bodies
}

/// T-R11：任何日志/错误消息都不得拼接密钥材料或明文字段。
#[test]
fn t_r11_logs_never_interpolate_key_material() {
    let root = src_tauri_root();
    let mut files = Vec::new();
    for dir in SCAN_DIRS {
        collect_rust_files(&root.join(dir), &mut files);
    }
    assert!(
        files.len() > 50,
        "扫描仅命中 {} 个 Rust 源文件，路径解析可能已失效",
        files.len()
    );

    let mut violations = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("读取 {} 失败: {}", file.display(), e));
        for body in log_macro_bodies(&text) {
            for token in FORBIDDEN_TOKENS {
                if body.contains(token) {
                    violations.push(format!(
                        "{}: {} 出现在日志宏内\n{}",
                        relative_to(file, &root),
                        token,
                        body
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "日志中禁止拼接密钥/明文：\n{}",
        violations.join("\n")
    );
}

/// 密钥类型即使被 `{:?}` 打印也只输出占位符。
#[test]
fn key_types_debug_output_is_redacted() {
    let dek = Dek::new([0xAB; 20]);
    let db_key = DbKey::new([0xCD; 32]);
    let kek = Kek::new([0xEF; 32]);
    let meta_key = MetaKey::new([0x12; 32]);

    for rendered in [
        format!("{:?}", dek),
        format!("{:?}", db_key),
        format!("{:?}", kek),
        format!("{:?}", meta_key),
    ] {
        assert!(
            rendered.contains("REDACTED"),
            "密钥 Debug 输出必须脱敏: {}",
            rendered
        );
        assert!(
            !rendered.contains("ababab")
                && !rendered.contains("cdcdcd")
                && !rendered.contains("efefef"),
            "密钥 Debug 输出不得包含真实字节: {}",
            rendered
        );
    }
}
