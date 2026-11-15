use std::fs;
use std::path::{Path, PathBuf};

const VAULT_PREFIX: &str = "vault_";
const COMMAND_ATTRIBUTE: &str = "#[command]";
const TAURI_COMMAND_ATTRIBUTE: &str = "#[tauri::command]";
const PRECONDITION_MARKER: &str = "// precondition:";
const GATE_CALL: &str = "with_unlocked(";
const DB_ACCESS: &str = "get_conn(";
const STATE_ACCESS: [&str; 4] = ["vault_state(", "sync_state(", "repo_state(", ".state::<"];
const PRECONDITIONS: [&str; 3] = ["None", "AnyState", "Unlocked"];

#[derive(Debug)]
struct CommandGate {
    name: String,
    file: String,
    precondition: Option<String>,
    body: String,
}

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src")
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

fn next_attribute(text: &str, from: usize) -> Option<usize> {
    let plain = text[from..].find(COMMAND_ATTRIBUTE).map(|i| from + i);
    let tauri = text[from..].find(TAURI_COMMAND_ATTRIBUTE).map(|i| from + i);
    match (plain, tauri) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn parse_fn_name(text: &str, from: usize) -> Option<(String, usize)> {
    let fn_at = text[from..].find("fn ")? + from;
    let rest = &text[fn_at + "fn ".len()..];
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        let end = fn_at + "fn ".len() + name.len();
        Some((name, end))
    }
}

fn extract_body(text: &str, from: usize) -> Option<(String, usize)> {
    let open = text[from..].find('{')? + from;
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((text[open..open + offset + 1].to_string(), open + offset + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_precondition(text: &str, attribute_pos: usize) -> Option<String> {
    let head = &text[..attribute_pos];
    for line in head.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = trimmed.strip_prefix(PRECONDITION_MARKER)?;
        return Some(value.trim().to_string());
    }
    None
}

fn parse_commands(text: &str, file: &str) -> Vec<CommandGate> {
    let mut commands = Vec::new();
    let mut cursor = 0;
    while let Some(attribute_pos) = next_attribute(text, cursor) {
        let (name, name_end) = match parse_fn_name(text, attribute_pos) {
            Some(found) => found,
            None => {
                commands.push(CommandGate {
                    name: format!("<unparsed@{}>", attribute_pos),
                    file: file.to_string(),
                    precondition: None,
                    body: String::new(),
                });
                cursor = attribute_pos + COMMAND_ATTRIBUTE.len();
                continue;
            }
        };
        let (body, body_end) = match extract_body(text, name_end) {
            Some(found) => found,
            None => {
                commands.push(CommandGate {
                    name: format!("<unparsed@{}>", attribute_pos),
                    file: file.to_string(),
                    precondition: None,
                    body: String::new(),
                });
                cursor = attribute_pos + COMMAND_ATTRIBUTE.len();
                continue;
            }
        };
        if name.starts_with(VAULT_PREFIX) {
            commands.push(CommandGate {
                precondition: extract_precondition(text, attribute_pos),
                name,
                file: file.to_string(),
                body,
            });
        }
        cursor = body_end;
    }
    commands
}

fn collected_commands() -> Vec<CommandGate> {
    let mut commands = Vec::new();
    for path in vault_source_files() {
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();
        let text = fs::read_to_string(&path).expect("vault command source file is readable");
        commands.extend(parse_commands(&text, &file));
    }
    commands
}

fn first_index(body: &str, needle: &str) -> Option<usize> {
    body.find(needle)
}

fn violations(command: &CommandGate) -> Vec<String> {
    let label = format!("{}::{}", command.file, command.name);
    let precondition = match command.precondition.as_deref() {
        Some(value) => value,
        None => return vec![format!("{}: missing `// precondition:` declaration", label)],
    };
    if !PRECONDITIONS.contains(&precondition) {
        return vec![format!(
            "{}: unknown precondition `{}`",
            label, precondition
        )];
    }
    let mut out = Vec::new();
    let gate = first_index(&command.body, GATE_CALL);
    let db_access = first_index(&command.body, DB_ACCESS);
    let touches_state = STATE_ACCESS
        .iter()
        .any(|token| command.body.contains(token));
    match precondition {
        "Unlocked" => {
            if gate.is_none() {
                out.push(format!(
                    "{}: `Unlocked` command has no `{}` gate call",
                    label, GATE_CALL
                ));
            }
            if let (Some(gate_at), Some(db_at)) = (gate, db_access) {
                if db_at < gate_at {
                    out.push(format!(
                        "{}: `{}` is reached before the `{}` gate",
                        label, DB_ACCESS, GATE_CALL
                    ));
                }
            }
        }
        "None" => {
            if gate.is_some() {
                out.push(format!(
                    "{}: `None` command must not call `{}`",
                    label, GATE_CALL
                ));
            }
            if touches_state {
                out.push(format!(
                    "{}: `None` command must not access vault state",
                    label
                ));
            }
            if db_access.is_some() {
                out.push(format!(
                    "{}: `None` command must not call `{}`",
                    label, DB_ACCESS
                ));
            }
        }
        "AnyState" if !touches_state => {
            out.push(format!(
                "{}: `AnyState` command does not access vault state",
                label
            ));
        }
        _ => {}
    }
    out
}

fn report(commands: &[CommandGate]) -> Vec<String> {
    let mut all: Vec<String> = commands.iter().flat_map(violations).collect();
    all.sort();
    all
}

#[test]
fn vault_commands_declare_precondition_in_place() {
    let commands = collected_commands();
    assert!(
        commands.len() >= 65,
        "vault command definitions are discoverable from cmd/vault sources (found {})",
        commands.len()
    );
    let missing: Vec<String> = commands
        .iter()
        .filter(|command| command.precondition.is_none())
        .map(|command| format!("{}::{}", command.file, command.name))
        .collect();
    assert!(
        missing.is_empty(),
        "commands without in-place `// precondition:` declaration: {:?}",
        missing
    );
}

#[test]
fn vault_commands_gate_matches_precondition() {
    let commands = collected_commands();
    let failures = report(&commands);
    assert!(
        failures.is_empty(),
        "vault command gate inconsistencies:\n{}",
        failures.join("\n")
    );
}

#[test]
fn gate_checker_rejects_ungated_unlocked_command() {
    let broken = CommandGate {
        name: "vault_broken".to_string(),
        file: "synthetic.rs".to_string(),
        precondition: Some("Unlocked".to_string()),
        body: "{ let state = super::vault_state(&app)?; Ok(()) }".to_string(),
    };
    assert!(
        !violations(&broken).is_empty(),
        "checker must reject an `Unlocked` command without a gate call"
    );

    let gated = CommandGate {
        name: "vault_gated".to_string(),
        file: "synthetic.rs".to_string(),
        precondition: Some("Unlocked".to_string()),
        body: "{ super::with_unlocked(&app, |db| { let _ = db; Ok(()) }) }".to_string(),
    };
    assert!(
        violations(&gated).is_empty(),
        "checker must accept a gated `Unlocked` command: {:?}",
        violations(&gated)
    );

    let leaking_none = CommandGate {
        name: "vault_leaking_none".to_string(),
        file: "synthetic.rs".to_string(),
        precondition: Some("None".to_string()),
        body: "{ let state = super::vault_state(&app)?; let _ = state; Ok(()) }".to_string(),
    };
    assert!(
        !violations(&leaking_none).is_empty(),
        "checker must reject a `None` command that touches vault state"
    );
}
