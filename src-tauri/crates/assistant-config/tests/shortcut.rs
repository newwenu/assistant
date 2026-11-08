use assistant_config::app_conf::default_keybindings;
use assistant_config::shortcut::resolve_bindings;
use std::collections::HashMap;

const SPECS: &[(&str, u8)] = &[("toggle_orb", 1), ("toggle_main", 2), ("new_chat", 3)];

fn parse(binding: &str) -> Option<String> {
    if binding == "NotAKey" {
        None
    } else {
        Some(binding.to_lowercase())
    }
}

#[test]
fn default_bindings_resolve() {
    let (resolved, issues) =
        resolve_bindings(SPECS, &HashMap::new(), &default_keybindings(), parse);
    assert_eq!(resolved.len(), SPECS.len());
    assert!(issues.is_empty());
    assert_eq!(resolved[0].id, "toggle_orb");
    assert_eq!(resolved[0].action, 1);
}

#[test]
fn configured_binding_overrides_default() {
    let mut configured = default_keybindings();
    configured.insert("new_chat".to_string(), "Ctrl+Alt+N".to_string());
    let (resolved, issues) = resolve_bindings(SPECS, &configured, &default_keybindings(), parse);
    assert!(issues.is_empty());
    let new_chat = resolved
        .iter()
        .find(|item| item.id == "new_chat")
        .expect("new_chat resolved");
    assert_eq!(new_chat.binding, "Ctrl+Alt+N");
}

#[test]
fn explicitly_cleared_binding_is_disabled() {
    let mut configured = default_keybindings();
    configured.insert("new_chat".to_string(), "   ".to_string());
    let (resolved, issues) = resolve_bindings(SPECS, &configured, &default_keybindings(), parse);
    assert_eq!(resolved.len(), SPECS.len() - 1);
    assert!(issues.is_empty());
    assert!(!resolved.iter().any(|item| item.id == "new_chat"));
}

#[test]
fn invalid_binding_is_reported() {
    let mut configured = default_keybindings();
    configured.insert("new_chat".to_string(), "NotAKey".to_string());
    let (resolved, issues) = resolve_bindings(SPECS, &configured, &default_keybindings(), parse);
    assert_eq!(resolved.len(), SPECS.len() - 1);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, "new_chat");
    assert_eq!(issues[0].error, "invalid binding");
    assert_eq!(issues[0].binding, "NotAKey");
}

#[test]
fn duplicate_binding_is_rejected() {
    let mut configured = default_keybindings();
    let orb_binding = configured
        .get("toggle_orb")
        .cloned()
        .expect("toggle_orb default");
    configured.insert("new_chat".to_string(), orb_binding);
    let (resolved, issues) = resolve_bindings(SPECS, &configured, &default_keybindings(), parse);
    assert_eq!(resolved.len(), SPECS.len() - 1);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, "new_chat");
    assert!(issues[0].error.starts_with("duplicate of toggle_orb"));
}

#[test]
fn missing_default_is_reported() {
    let mut defaults = default_keybindings();
    defaults.remove("new_chat");
    let (resolved, issues) = resolve_bindings(SPECS, &HashMap::new(), &defaults, parse);
    assert_eq!(resolved.len(), SPECS.len() - 1);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, "new_chat");
    assert_eq!(issues[0].error, "no default binding");
    assert!(issues[0].binding.is_empty());
}
