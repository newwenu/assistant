use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const KEY_PREFIX: &str = "vault_";
const ACCESS_MARKER: &str = ".vault_";
const DECLARATION_MARKER: &str = "pub vault_";

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src")
}

fn vault_conf_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../assistant-config/src/vault_conf.rs")
}

fn vault_source_files() -> Vec<PathBuf> {
    let dir = src_root().join("cmd").join("vault");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("vault command source directory is readable")
        .map(|entry| {
            entry
                .expect("vault command source entry is readable")
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();
    files.sort();
    files
}

fn identifier_at(text: &str, start: usize) -> String {
    text[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect()
}

fn is_method_call(text: &str, name_start: usize, name_len: usize) -> bool {
    text[name_start + name_len..].starts_with('(')
}

fn accessed_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for file in vault_source_files() {
        let text = fs::read_to_string(&file).expect("vault command source file is readable");
        let mut cursor = 0;
        while let Some(offset) = text[cursor..].find(ACCESS_MARKER) {
            let start = cursor + offset + ACCESS_MARKER.len();
            let name = identifier_at(&text, start);
            if !name.is_empty() && !is_method_call(&text, start, name.len()) {
                keys.push(format!("{KEY_PREFIX}{name}"));
            }
            cursor = start;
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

fn declared_keys() -> Vec<String> {
    let text = fs::read_to_string(vault_conf_source()).expect("vault_conf.rs is readable");
    let mut keys = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(DECLARATION_MARKER) {
            continue;
        }
        let name = identifier_at(trimmed, "pub ".len());
        if !name.is_empty() {
            keys.push(name);
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

fn missing_keys(accessed: &[String], declared: &[String]) -> Vec<String> {
    let known: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    let mut out: Vec<String> = accessed
        .iter()
        .filter(|key| !known.contains(key.as_str()))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn vault_conf_keys_used_by_commands_are_declared() {
    let accessed = accessed_keys();
    let declared = declared_keys();
    assert!(
        declared.len() > 1,
        "vault config keys are discoverable from vault_conf.rs"
    );
    assert!(
        accessed.len() > 1,
        "vault config key accesses are discoverable from cmd sources"
    );
    assert_eq!(
        missing_keys(&accessed, &declared),
        Vec::<String>::new(),
        "cmd sources access vault config keys missing from VaultConf"
    );
}

#[test]
fn check_reports_an_undeclared_key() {
    let declared = vec![
        format!("{}alpha", KEY_PREFIX),
        format!("{}beta", KEY_PREFIX),
    ];
    let accessed = vec![
        format!("{}alpha", KEY_PREFIX),
        format!("{}phantom", KEY_PREFIX),
    ];
    assert_eq!(
        missing_keys(&accessed, &declared),
        vec![format!("{}phantom", KEY_PREFIX)],
        "referencing an undeclared key must be reported"
    );
}

#[test]
fn check_reports_a_declared_key_without_readers() {
    let declared = vec![format!("{}alpha", KEY_PREFIX)];
    let accessed = vec![format!("{}alpha", KEY_PREFIX)];
    assert_eq!(
        missing_keys(&declared, &accessed),
        Vec::<String>::new(),
        "declared keys without readers are allowed"
    );
    assert_eq!(
        missing_keys(&accessed, &[]),
        vec![format!("{}alpha", KEY_PREFIX)],
        "an empty declaration set must reject every access"
    );
}
