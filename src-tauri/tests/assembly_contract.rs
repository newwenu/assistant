// 参考: docs/store/I24-feature-extension-spec.md §3.8.5 — 装配契约的六条集合断言

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REGISTRATION_OPEN: &str = "generate_handler![";
const CFG_PREFIX: &str = "#[cfg(";
const CFG_FEATURE_MARKER: &str = "feature = \"";
const CFG_ALLOWED_IN: &str = "cmd/mod.rs";
const PARSER_FLOOR: usize = 120;
const VAULT_CFG_LIMIT: usize = 6;
const LIB_RS_LINE_LIMIT: usize = 40;
const CLEANUP_FN: &str = "fn vault_cleanup_on_exit";

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

fn read(relative: &str) -> String {
    let path = src_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {}", relative, e))
}

fn plugin_registry_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(src_root()).expect("source root is readable");
    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        let is_rust = path.extension().and_then(|ext| ext.to_str()) == Some("rs");
        let is_plugin = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.ends_with("_plugin"))
            .unwrap_or(false);
        if is_rust && is_plugin {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn plugin_registry_names() -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for file in plugin_registry_files() {
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{} is readable: {}", file.display(), e));
        names.extend(registered_names(&text)?);
    }
    Ok(names)
}

fn registered_all() -> Result<Vec<String>, String> {
    let mut names = registered_names(&read("ipc.rs"))?;
    names.extend(plugin_registry_names()?);
    Ok(names)
}

fn strip_rust_comments(text: &str) -> String {
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

fn registration_bodies(text: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find(REGISTRATION_OPEN) {
        let start = cursor + offset + REGISTRATION_OPEN.len();
        let mut depth = 1usize;
        let mut close = None;
        for (index, ch) in text[start..].char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(start + index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = close.unwrap_or(text.len());
        bodies.push(text[start..end].to_string());
        if end >= text.len() {
            break;
        }
        cursor = end + 1;
    }
    bodies
}

fn is_valid_entry(entry: &str) -> bool {
    !entry.is_empty()
        && entry
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
}

fn registered_names(text: &str) -> Result<Vec<String>, String> {
    let cleaned = strip_rust_comments(text);
    let mut names = Vec::new();
    for body in registration_bodies(&cleaned) {
        for entry in body.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !is_valid_entry(trimmed) {
                return Err(format!("注册表条目不是纯标识符路径: {:?}", trimmed));
            }
            let segment = trimmed.rsplit("::").next().unwrap_or(trimmed);
            names.push(segment.to_string());
        }
    }
    Ok(names)
}

fn declared_names() -> BTreeSet<String> {
    let raw = fs::read_to_string(manifest_dir().join("commands.json"))
        .expect("commands.json is readable");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("commands.json is valid json");
    let groups = value
        .get("groups")
        .and_then(|v| v.as_object())
        .expect("commands.json has groups object");
    let mut names = BTreeSet::new();
    for (group, list) in groups {
        let list = list
            .as_array()
            .unwrap_or_else(|| panic!("commands.json group {} is an array", group));
        for item in list {
            let name = item
                .as_str()
                .unwrap_or_else(|| panic!("commands.json group {} holds strings", group));
            assert!(
                names.insert(name.to_string()),
                "commands.json 中命令 {} 重复声明",
                name
            );
        }
    }
    names
}

fn shorten_paths(text: &str) -> String {
    fn flush(token: &mut String, out: &mut String) {
        if token.is_empty() {
            return;
        }
        match token.rfind("::") {
            Some(idx) => out.push_str(&token[idx + 2..]),
            None => out.push_str(token),
        }
        token.clear();
    }

    let mut out = String::new();
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
            out.push(ch);
        }
    }
    flush(&mut token, &mut out);
    out
}

fn normalized_signature(text: &str, file: &str) -> String {
    let start = text
        .find(CLEANUP_FN)
        .unwrap_or_else(|| panic!("{} 缺少 {}", file, CLEANUP_FN));
    let after_name = &text[start + CLEANUP_FN.len()..];
    let open = after_name
        .find('(')
        .unwrap_or_else(|| panic!("{} 的 {} 缺少参数表", file, CLEANUP_FN));
    let body = &after_name[open + 1..];
    let mut depth = 1usize;
    let mut close = None;
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let params = &body[..close.unwrap_or_else(|| panic!("{} 的参数表未闭合", file))];
    let tail = &body[close.unwrap() + 1..];
    assert!(
        !tail.trim_start().starts_with("->"),
        "{} 的 {} 返回类型与另一侧不一致",
        file,
        CLEANUP_FN
    );

    let mut normalized = Vec::new();
    for param in params.split(',') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }
        let (name, ty) = param
            .split_once(':')
            .unwrap_or_else(|| panic!("{} 的参数缺少类型: {}", file, param));
        let name = name.trim().trim_start_matches('_');
        let ty = shorten_paths(ty).split_whitespace().collect::<String>();
        normalized.push(format!("{}:{}", name, ty));
    }
    normalized.join(",")
}

fn cfg_feature_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(CFG_PREFIX) {
            continue;
        }
        if let Some(name) = extract_feature_name(trimmed) {
            names.push(name);
        }
    }
    names
}

fn extract_feature_name(line: &str) -> Option<String> {
    let start = line.find(CFG_FEATURE_MARKER)? + CFG_FEATURE_MARKER.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn cfg_location_allowed(feature: &str, relative: &str) -> bool {
    if relative == CFG_ALLOWED_IN {
        return true;
    }
    let expected = format!("{}_plugin.rs", feature.replace('-', "_"));
    relative.rsplit('/').next() == Some(expected.as_str())
}

#[test]
fn a1_registry_file_has_no_cfg() {
    let ipc = read("ipc.rs");
    assert!(
        !ipc.contains("#[cfg"),
        "ipc.rs 是纯核心注册表，不得出现任何 #[cfg]"
    );
}

#[test]
fn a2_feature_cfg_only_in_boundary_files() {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut offenders = Vec::new();
    let mut vault_total = 0usize;
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        let relative = file
            .strip_prefix(src_root())
            .expect("file lives under src")
            .to_string_lossy()
            .replace('\\', "/");
        for feature in cfg_feature_names(&text) {
            if !cfg_location_allowed(&feature, &relative) {
                offenders.push(format!("{} -> feature \"{}\"", relative, feature));
            }
            if feature == "vault" {
                vault_total += 1;
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "特性 #[cfg] 只能出现在 {} 或与特性同名的 *_plugin.rs，越界: {:?}",
        CFG_ALLOWED_IN,
        offenders
    );
    assert!(
        vault_total <= VAULT_CFG_LIMIT,
        "全后端 vault 特性 #[cfg] 共 {} 行，超过上限 {}",
        vault_total,
        VAULT_CFG_LIMIT
    );
}

#[test]
fn a3_lib_rs_is_assembly_only() {
    let lib = read("lib.rs");
    let lines = lib.lines().count();
    assert!(
        lines <= LIB_RS_LINE_LIMIT,
        "lib.rs 共 {} 行，超过装配层上限 {}",
        lines,
        LIB_RS_LINE_LIMIT
    );
    assert!(
        !lib.contains(REGISTRATION_OPEN),
        "lib.rs 不得内联 generate_handler! 注册表"
    );
}

#[test]
fn a4_registered_and_declared_sets_match_both_ways() {
    let registered: BTreeSet<String> = registered_all()
        .expect("注册表可解析")
        .into_iter()
        .collect();
    let declared = declared_names();

    let missing_declaration: Vec<&String> = registered.difference(&declared).collect();
    let missing_registration: Vec<&String> = declared.difference(&registered).collect();

    assert!(
        missing_declaration.is_empty(),
        "已注册但未在 commands.json 声明: {:?}",
        missing_declaration
    );
    assert!(
        missing_registration.is_empty(),
        "已在 commands.json 声明但未注册: {:?}",
        missing_registration
    );
}

#[test]
fn a5_parser_floor() {
    let total = registered_all().expect("注册表可解析").len();
    assert!(
        total >= PARSER_FLOOR,
        "注册表仅解析到 {} 条命令，低于下限 {}，解析器可能已失效",
        total,
        PARSER_FLOOR
    );
}

#[test]
fn a6_three_state_classification_and_stub_signature() {
    let core = registered_names(&read("ipc.rs")).expect("ipc.rs 注册表可解析");
    let plugin = plugin_registry_names().expect("插件注册表可解析");
    let declared = declared_names();

    let core_set: BTreeSet<&str> = core.iter().map(String::as_str).collect();
    let plugin_set: BTreeSet<&str> = plugin.iter().map(String::as_str).collect();
    let host_only: BTreeSet<&str> = declared
        .iter()
        .filter(|name| !core_set.contains(name.as_str()) && !plugin_set.contains(name.as_str()))
        .map(String::as_str)
        .collect();

    let mut stray = Vec::new();
    for name in &declared {
        let hits = [
            core_set.contains(name.as_str()),
            plugin_set.contains(name.as_str()),
            host_only.contains(name.as_str()),
        ]
        .into_iter()
        .filter(|hit| *hit)
        .count();
        if hits != 1 {
            stray.push(format!("{} (命中 {} 态)", name, hits));
        }
    }
    assert!(
        stray.is_empty(),
        "每条命令必须恰好命中 core / plugin / hostOnly 之一: {:?}",
        stray
    );

    let stub = normalized_signature(&read("cmd/mod.rs"), "cmd/mod.rs");
    let real = normalized_signature(&read("cmd/vault/mod.rs"), "cmd/vault/mod.rs");
    assert_eq!(
        stub, real,
        "vault_cleanup_on_exit 的桩与真实现签名归一化后必须一致"
    );
}
