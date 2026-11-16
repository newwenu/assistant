use std::fs;
use std::path::{Path, PathBuf};

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

const SCAN_DIRS: [&str; 3] = [
    "crates/assistant-vault/src",
    "crates/assistant-crypto/src",
    "src/cmd/vault",
];

const V1_SYMBOLS: [&str; 4] = [
    "BOOTSTRAP_SALT",
    "vault_crypto_info",
    "master_key_salt",
    "emergency",
];

const ALLOWED_V1_RESIDUE: [(&str, &str); 2] = [
    (
        "crates/assistant-vault/src/vault_file/meta.rs",
        "master_key_salt",
    ),
    ("crates/assistant-vault/src/vault_file/meta.rs", "emergency"),
];

#[test]
fn v1_residue_symbols_stay_within_allowlist() {
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
        let rel = relative_to(file, &root);
        for symbol in V1_SYMBOLS {
            if !text.contains(symbol) {
                continue;
            }
            let allowed = ALLOWED_V1_RESIDUE
                .iter()
                .any(|(path, allowed_symbol)| *path == rel && *allowed_symbol == symbol);
            if !allowed {
                violations.push(format!("{}: {}", rel, symbol));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "发现未登记的白名单外 v1 残留符号: {:?}",
        violations
    );

    for (rel, symbol) in ALLOWED_V1_RESIDUE {
        let path = root.join(rel);
        assert!(path.is_file(), "白名单指向的文件不存在: {}", rel);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("读取 {} 失败: {}", path.display(), e));
        assert!(
            text.contains(symbol),
            "白名单条目已失效：{} 中已无 {}，请同步删除该条目",
            rel,
            symbol
        );
    }
}

trait AmbiguousIfClone<A> {
    fn probe() {}
}

impl<T: ?Sized> AmbiguousIfClone<()> for T {}

impl<T: Clone> AmbiguousIfClone<u8> for T {}

#[test]
fn key_types_do_not_implement_clone() {
    let _ = <assistant_crypto::vault_crypto::types::Dek as AmbiguousIfClone<_>>::probe;
    let _ = <assistant_crypto::vault_crypto::types::Kek as AmbiguousIfClone<_>>::probe;
    let _ = <assistant_crypto::vault_crypto::types::DbKey as AmbiguousIfClone<_>>::probe;
    let _ = <assistant_crypto::vault_crypto::types::MetaKey as AmbiguousIfClone<_>>::probe;
    let _ = <assistant_crypto::vault_crypto::types::PerEntryKey as AmbiguousIfClone<_>>::probe;
}

const DECRYPT_PATH_FILES: [&str; 2] = [
    "crates/assistant-vault/src/security_db/entries/field_crypto.rs",
    "crates/assistant-crypto/src/vault_crypto/mod.rs",
];

#[test]
fn decrypt_paths_do_not_swallow_errors() {
    let root = src_tauri_root();
    let mut violations = Vec::new();
    for rel in DECRYPT_PATH_FILES {
        let path = root.join(rel);
        assert!(path.is_file(), "解密路径文件不存在: {}", rel);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("读取 {} 失败: {}", path.display(), e));
        if text.contains("unwrap_or_default") {
            violations.push(rel);
        }
    }
    assert!(
        violations.is_empty(),
        "解密路径不得用 unwrap_or_default 吞掉错误: {:?}",
        violations
    );
}
