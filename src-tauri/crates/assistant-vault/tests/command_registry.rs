use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const VAULT_PREFIX: &str = "vault_";
const COMMAND_ATTRIBUTE: &str = "#[command]";
const TAURI_COMMAND_ATTRIBUTE: &str = "#[tauri::command]";
const REGISTRATION_OPEN: &str = "generate_handler![";
const DEFINITION_LOOKAHEAD: usize = 8;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("command source directory is readable");
    for entry in entries {
        let path = entry.expect("command source entry is readable").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn parse_fn_name(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("pub ")?;
    let rest = rest.strip_prefix("async ").unwrap_or(rest);
    let rest = rest.strip_prefix("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn declared_commands() -> Vec<String> {
    let mut files = Vec::new();
    collect_rs_files(&src_root().join("cmd"), &mut files);
    let mut names = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("command source file is readable");
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with(COMMAND_ATTRIBUTE)
                && !trimmed.starts_with(TAURI_COMMAND_ATTRIBUTE)
            {
                continue;
            }
            let window_end = (index + 1 + DEFINITION_LOOKAHEAD).min(lines.len());
            let found = lines[index + 1..window_end]
                .iter()
                .find_map(|candidate| parse_fn_name(candidate));
            if let Some(name) = found {
                if name.starts_with(VAULT_PREFIX) {
                    names.push(name);
                }
            }
        }
    }
    names
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

fn strip_attributes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '#' && chars.peek() == Some(&'[') {
            chars.next();
            let mut depth = 1usize;
            for c in chars.by_ref() {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push(' ');
            continue;
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

fn entry_name(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return None;
    }
    let segment = trimmed.rsplit("::").next().unwrap_or(trimmed);
    let valid = !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if valid {
        Some(segment.to_string())
    } else {
        None
    }
}

fn registered_commands() -> Vec<String> {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);
    let mut names = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("registration source file is readable");
        for body in registration_bodies(&text) {
            let cleaned = strip_attributes(&strip_rust_comments(&body));
            for entry in cleaned.split(',') {
                if let Some(name) = entry_name(entry) {
                    if name.starts_with(VAULT_PREFIX) {
                        names.push(name);
                    }
                }
            }
        }
    }
    names
}

fn absent_from(candidates: &[String], reference: &[String]) -> Vec<String> {
    let known: BTreeSet<&str> = reference.iter().map(String::as_str).collect();
    let mut out: Vec<String> = candidates
        .iter()
        .filter(|name| !known.contains(name.as_str()))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn vault_commands_and_registrations_match_both_ways() {
    let declared = declared_commands();
    let registered = registered_commands();
    assert!(
        declared.len() > 1,
        "vault command definitions are discoverable from cmd sources"
    );
    assert!(
        registered.len() > 1,
        "vault command registrations are discoverable from generate_handler! blocks"
    );
    assert_eq!(
        absent_from(&declared, &registered),
        Vec::<String>::new(),
        "#[command] vault functions are missing from generate_handler!"
    );
    assert_eq!(
        absent_from(&registered, &declared),
        Vec::<String>::new(),
        "generate_handler! registers vault commands without a #[command] definition"
    );
}

#[test]
fn vault_command_names_are_unique() {
    let declared = declared_commands();
    let mut seen = BTreeSet::new();
    let duplicates: Vec<&String> = declared
        .iter()
        .filter(|name| !seen.insert(name.as_str()))
        .collect();
    assert!(
        duplicates.is_empty(),
        "duplicate #[command] names would shadow each other: {duplicates:?}"
    );
}

#[test]
fn check_reports_a_dropped_registration() {
    let declared = vec![
        format!("{}alpha", VAULT_PREFIX),
        format!("{}beta", VAULT_PREFIX),
    ];
    let registered = vec![format!("{}alpha", VAULT_PREFIX)];
    assert_eq!(
        absent_from(&declared, &registered),
        vec![format!("{}beta", VAULT_PREFIX)],
        "dropping one registration must be reported as a missing registration"
    );
}

#[test]
fn check_reports_an_unregistered_definition() {
    let declared = vec![format!("{}alpha", VAULT_PREFIX)];
    let registered = vec![
        format!("{}alpha", VAULT_PREFIX),
        format!("{}phantom_probe", VAULT_PREFIX),
    ];
    assert_eq!(
        absent_from(&registered, &declared),
        vec![format!("{}phantom_probe", VAULT_PREFIX)],
        "an extra registration without a definition must be reported"
    );
}
