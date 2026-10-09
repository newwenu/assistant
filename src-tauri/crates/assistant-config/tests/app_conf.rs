use assistant_config::app_conf::AppConf;

fn conf_path() -> std::path::PathBuf {
    tempfile::tempdir().unwrap().path().join("config.json")
}

#[test]
fn new_returns_valid_and_matches_default() {
    let conf = AppConf::new();
    assert_eq!(conf.theme, "system");
    assert_eq!(conf.language, "zh-CN");
    assert_eq!(conf.hotkey, "");
    assert!(!conf.stay_on_top);
    assert!(conf.close_to_tray);
    assert_eq!(conf.keybindings.len(), 3);
    assert!(conf.validate().is_ok());

    let default = AppConf::default();
    assert_eq!(conf.theme, default.theme);
    assert_eq!(conf.keybindings, default.keybindings);
}

#[test]
fn validate_theme_and_language_rules() {
    let mut conf = AppConf::new();
    for theme in &["system", "light", "dark"] {
        conf.theme = theme.to_string();
        assert!(conf.validate().is_ok());
    }
    conf.theme = "neon".to_string();
    assert!(conf.validate().is_err());

    conf.theme = "system".to_string();
    conf.language = String::new();
    assert!(conf.validate().is_err());
    conf.language = "en-US".to_string();
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
}

#[test]
fn deserialize_missing_optional_fields_use_defaults() {
    let json = r#"{"theme":"dark","language":"en","hotkey":"","stay_on_top":true}"#;
    let conf: AppConf = serde_json::from_str(json).unwrap();

    assert_eq!(conf.theme, "dark");
    assert!(!conf.launch_at_login);
    assert!(!conf.close_to_tray);
    assert!(conf.keybindings.is_empty());
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
    let path = conf_path();
    let conf = AppConf::load_from(&path).unwrap();
    assert_eq!(conf.theme, "system");
    assert!(path.exists());
}

#[test]
fn save_to_and_load_from_roundtrip() {
    let path = conf_path();
    let mut conf = AppConf::new();
    conf.theme = "dark".to_string();
    conf.language = "en-US".to_string();
    conf.stay_on_top = true;

    conf.save_to(&path).unwrap();
    let loaded = AppConf::load_from(&path).unwrap();

    assert_eq!(loaded.theme, "dark");
    assert_eq!(loaded.language, "en-US");
    assert!(loaded.stay_on_top);
}

#[test]
fn load_from_falls_back_on_corrupt_file() {
    let path = conf_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"not json at all").unwrap();

    let loaded = AppConf::load_from(&path).unwrap();
    assert_eq!(loaded.theme, "system");
}

#[test]
fn load_from_recovers_from_temp_file() {
    let path = conf_path();
    let conf = AppConf::new();
    conf.save_to(&path).unwrap();

    let temp = path.with_extension("tmp");
    let mut modified = conf.clone();
    modified.theme = "dark".to_string();
    let json = serde_json::to_string_pretty(&modified).unwrap();
    std::fs::write(&temp, json.as_bytes()).unwrap();

    let loaded = AppConf::load_from(&path).unwrap();
    assert_eq!(loaded.theme, "dark");
    assert!(!temp.exists());
}
