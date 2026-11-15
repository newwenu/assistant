mod common;

use assistant_config::app_conf::{AppConf, ThemeMode};
use common::conf_path;

#[test]
fn new_returns_valid_and_matches_default() {
    let conf = AppConf::new();
    assert_eq!(conf.theme, ThemeMode::System);
    assert_eq!(conf.language, "zh-CN");
    assert_eq!(conf.hotkey, "");
    assert!(!conf.stay_on_top);
    assert!(conf.close_to_tray);
    assert_eq!(conf.keybindings.len(), 3);
    assert!(conf.auto_compact);
    assert!(conf.validate().is_ok());

    let default = AppConf::default();
    assert_eq!(conf.theme, default.theme);
    assert_eq!(conf.keybindings, default.keybindings);
    assert_eq!(conf.auto_compact, default.auto_compact);
}

#[test]
fn validate_language_rule() {
    let mut conf = AppConf::new();
    conf.language = String::new();
    assert!(conf.validate().is_err());
    conf.language = "en-US".to_string();
    assert!(conf.validate().is_ok());
}

#[test]
fn deserialize_rejects_unknown_theme() {
    let json = r#"{"theme":"neon","language":"en","hotkey":"","stay_on_top":true}"#;
    assert!(
        serde_json::from_str::<AppConf>(json).is_err(),
        "theme 是封闭集合，未知值必须在反序列化阶段被拒绝"
    );
}

#[test]
fn validate_agent_tool_rules() {
    let mut conf = AppConf::new();
    assert!(conf.validate().is_ok(), "两项都留空是合法状态");

    conf.approval_timeout_secs = Some(0);
    assert!(conf.validate().is_err(), "零超时既非放行也非等待，必须拒绝");
    conf.approval_timeout_secs = Some(5);
    assert!(conf.validate().is_ok());
}

#[test]
fn serialize_deserialize_roundtrip() {
    let original = AppConf::new();
    let json = serde_json::to_string(&original).unwrap();
    let restored: AppConf = serde_json::from_str(&json).unwrap();

    assert_eq!(original.theme, restored.theme);
    assert_eq!(original.language, restored.language);
    assert_eq!(original.hotkey, restored.hotkey);
    assert_eq!(original.stay_on_top, restored.stay_on_top);
    assert_eq!(original.launch_at_login, restored.launch_at_login);
    assert_eq!(original.close_to_tray, restored.close_to_tray);
    assert_eq!(original.keybindings, restored.keybindings);
    assert_eq!(original.auto_compact, restored.auto_compact);
    assert_eq!(
        original.compact_threshold_percent_of_budget,
        restored.compact_threshold_percent_of_budget
    );
    assert_eq!(
        original.compact_summary_max_tokens,
        restored.compact_summary_max_tokens
    );
}

#[test]
fn deserialize_missing_optional_fields_use_defaults() {
    let json = r#"{"theme":"dark","language":"en","hotkey":"","stay_on_top":true}"#;
    let conf: AppConf = serde_json::from_str(json).unwrap();

    assert_eq!(conf.theme, ThemeMode::Dark);
    assert!(!conf.launch_at_login);
    assert!(!conf.close_to_tray);
    assert!(conf.keybindings.is_empty());
    assert!(conf.auto_compact);
}

#[test]
fn migrate_is_noop() {
    let conf = AppConf::new();
    let json = serde_json::to_string(&conf).unwrap();
    let loaded: AppConf = serde_json::from_str(&json).unwrap();
    assert_eq!(conf.keybindings, loaded.keybindings);
}

#[test]
fn get_conf_path_under_app_dir() {
    let dir = AppConf::get_conf_dir().unwrap();
    let path = AppConf::get_conf_path().unwrap();
    assert!(dir.to_string_lossy().contains("com.assistant"));
    assert!(path.starts_with(&dir));
    assert!(path.to_string_lossy().ends_with("config.json"));
}

#[test]
fn load_from_creates_default_when_missing() {
    let (_dir, path) = conf_path("config.json");
    let conf = AppConf::load_from(&path).unwrap();
    assert_eq!(conf.theme, ThemeMode::System);
    assert!(path.exists());
}

#[test]
fn save_to_and_load_from_roundtrip() {
    let (_dir, path) = conf_path("config.json");
    let mut conf = AppConf::new();
    conf.theme = ThemeMode::Dark;
    conf.language = "en-US".to_string();
    conf.stay_on_top = true;

    conf.save_to(&path).unwrap();
    let loaded = AppConf::load_from(&path).unwrap();

    assert_eq!(loaded.theme, ThemeMode::Dark);
    assert_eq!(loaded.language, "en-US");
    assert!(loaded.stay_on_top);
}

#[test]
fn load_from_falls_back_on_corrupt_file() {
    let (_dir, path) = conf_path("config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"not json at all").unwrap();

    let loaded = AppConf::load_from(&path).unwrap();
    assert_eq!(loaded.theme, ThemeMode::System);
}

#[test]
fn load_from_recovers_from_temp_file() {
    let (_dir, path) = conf_path("config.json");
    let conf = AppConf::new();
    conf.save_to(&path).unwrap();

    let temp = path.with_extension("tmp");
    let mut modified = conf.clone();
    modified.theme = ThemeMode::Dark;
    let json = serde_json::to_string_pretty(&modified).unwrap();
    std::fs::write(&temp, json.as_bytes()).unwrap();

    let loaded = AppConf::load_from(&path).unwrap();
    assert_eq!(loaded.theme, ThemeMode::Dark);
    assert!(!temp.exists());
}

#[test]
fn legacy_threshold_key_is_ignored_after_migration_window() {
    let (_dir, path) = conf_path("config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut json = serde_json::to_value(AppConf::new()).unwrap();
    json["compact_threshold_percent"] = serde_json::json!(50);
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let loaded = AppConf::load_from(&path).unwrap();
    assert_eq!(
        loaded.compact_threshold_percent_of_budget,
        AppConf::new().compact_threshold_percent_of_budget
    );

    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(!rewritten.contains("compact_threshold_percent\""));
}

#[test]
fn amend_single_field() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "theme": "dark" });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.theme, ThemeMode::Dark);
    assert_eq!(merged.language, "zh-CN");
    assert!(!merged.stay_on_top);
}

#[test]
fn amend_multiple_fields() {
    let conf = AppConf::new();
    let patch =
        serde_json::json!({ "theme": "light", "stay_on_top": true, "launch_at_login": true });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.theme, ThemeMode::Light);
    assert!(merged.stay_on_top);
    assert!(merged.launch_at_login);
    assert_eq!(merged.language, "zh-CN");
}

#[test]
fn amend_preserves_unpatched_fields() {
    let mut conf = AppConf::new();
    conf.language = "en-US".to_string();
    conf.close_to_tray = false;
    let patch = serde_json::json!({ "theme": "dark" });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.theme, ThemeMode::Dark);
    assert_eq!(merged.language, "en-US");
    assert!(!merged.close_to_tray);
}

#[test]
fn amend_rejects_unknown_theme() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "theme": "neon" });
    let err = conf.amend(patch).unwrap_err();
    assert!(err.contains("neon"), "unexpected error: {}", err);
    for variant in ["system", "light", "dark"] {
        assert!(err.contains(variant), "unexpected error: {}", err);
    }
}

#[test]
fn amend_type_mismatch_returns_error() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "stay_on_top": "not_a_bool" });
    assert!(conf.amend(patch).is_err());
}

#[test]
fn amend_unknown_key_rejected() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "nonexistent": 42, "theme": "dark" });
    let err = conf.amend(patch).unwrap_err();
    assert!(err.contains("nonexistent"), "unexpected error: {}", err);
}

#[test]
fn amend_camel_case_key_rejected() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "stayOnTop": true });
    let err = conf.amend(patch).unwrap_err();
    assert!(err.contains("stayOnTop"), "unexpected error: {}", err);
}

#[test]
fn amend_open_map_keys_accepted() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "keybindings": { "custom_shortcut": "Ctrl+K" } });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.keybindings.get("custom_shortcut").unwrap(), "Ctrl+K");
}

#[test]
fn amend_keybindings() {
    let conf = AppConf::new();
    let patch = serde_json::json!({ "keybindings": { "toggle_orb": "Ctrl+O" } });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.keybindings.get("toggle_orb").unwrap(), "Ctrl+O");
}

#[test]
fn amend_compact_fields() {
    let conf = AppConf::new();
    let patch = serde_json::json!({
        "auto_compact": false,
        "compact_threshold_percent_of_budget": 50,
        "compact_summary_prompt": "Summarize briefly"
    });
    let merged = conf.amend(patch).unwrap();
    assert!(!merged.auto_compact);
    assert_eq!(merged.compact_threshold_percent_of_budget, 50);
    assert_eq!(
        merged.compact_summary_prompt.as_deref(),
        Some("Summarize briefly")
    );
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn serde_rejects_unknown_theme(theme in "[a-zA-Z]{1,10}") {
            let mut value = serde_json::to_value(AppConf::new()).unwrap();
            value["theme"] = serde_json::json!(theme.clone());
            let json = serde_json::to_string(&value).unwrap();
            let valid = ["system", "light", "dark"];
            assert_eq!(
                serde_json::from_str::<AppConf>(&json).is_ok(),
                valid.contains(&theme.as_str())
            );
        }

        #[test]
        fn serde_roundtrip_preserves_validate(language in "[a-zA-Z-]{1,10}") {
            let mut conf = AppConf::new();
            conf.language = language;
            let json = serde_json::to_string(&conf).unwrap();
            let decoded: AppConf = serde_json::from_str(&json).unwrap();
            assert_eq!(conf.validate().is_ok(), decoded.validate().is_ok());
        }
    }
}
