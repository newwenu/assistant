use assistant_config::orb_conf::OrbConf;

fn conf_path() -> std::path::PathBuf {
    tempfile::tempdir().unwrap().path().join("orb-config.json")
}

#[test]
fn new_returns_valid_and_matches_default() {
    let conf = OrbConf::new();
    assert!(!conf.click_through);
    assert!(conf.always_on_top);
    assert_eq!(conf.scale_percent, 100);
    assert_eq!(conf.opacity_percent, 100);
    assert_eq!(conf.idle_opacity_percent, 85);
    assert_eq!(conf.hover_delay_ms, 0);
    assert!(conf.position_x.is_none());
    assert!(conf.position_y.is_none());
    assert!(conf.validate().is_ok());

    let default = OrbConf::default();
    assert_eq!(conf.scale_percent, default.scale_percent);
    assert_eq!(conf.opacity_percent, default.opacity_percent);
    assert_eq!(conf.idle_opacity_percent, default.idle_opacity_percent);
}

#[test]
fn validate_scale_bounds() {
    let mut conf = OrbConf::new();
    conf.scale_percent = 9;
    assert!(conf.validate().is_err());
    conf.scale_percent = 10;
    assert!(conf.validate().is_ok());
    conf.scale_percent = 300;
    assert!(conf.validate().is_ok());
    conf.scale_percent = 301;
    assert!(conf.validate().is_err());
}

#[test]
fn validate_opacity_bounds() {
    let mut conf = OrbConf::new();
    conf.opacity_percent = 9;
    assert!(conf.validate().is_err());
    conf.opacity_percent = 10;
    assert!(conf.validate().is_ok());
    conf.opacity_percent = 100;
    assert!(conf.validate().is_ok());
    conf.opacity_percent = 101;
    assert!(conf.validate().is_err());
}

#[test]
fn validate_hover_delay_bounds() {
    let mut conf = OrbConf::new();
    conf.hover_delay_ms = 0;
    assert!(conf.validate().is_ok());
    conf.hover_delay_ms = 5000;
    assert!(conf.validate().is_ok());
    conf.hover_delay_ms = 5001;
    assert!(conf.validate().is_err());
}

#[test]
fn validate_idle_opacity_bounds() {
    let mut conf = OrbConf::new();
    conf.idle_opacity_percent = 9;
    assert!(conf.validate().is_err());
    conf.idle_opacity_percent = 10;
    assert!(conf.validate().is_ok());
    conf.idle_opacity_percent = 100;
    assert!(conf.validate().is_ok());
    conf.idle_opacity_percent = 101;
    assert!(conf.validate().is_err());
}

#[test]
fn serialize_deserialize_roundtrip() {
    let original = OrbConf::new();
    let json = serde_json::to_string(&original).unwrap();
    let restored: OrbConf = serde_json::from_str(&json).unwrap();

    assert_eq!(original.scale_percent, restored.scale_percent);
    assert_eq!(original.opacity_percent, restored.opacity_percent);
    assert_eq!(original.idle_opacity_percent, restored.idle_opacity_percent);
    assert_eq!(original.position_x, restored.position_x);
    assert_eq!(original.position_y, restored.position_y);
}

#[test]
fn deserialize_missing_positional_fields() {
    let json = r#"{
        "click_through": false,
        "always_on_top": true,
        "scale_percent": 100,
        "opacity_percent": 100,
        "hide_on_pointer_hover": false,
        "hover_delay_ms": 0
    }"#;
    let conf: OrbConf = serde_json::from_str(json).unwrap();

    assert!(conf.position_x.is_none());
    assert!(conf.position_y.is_none());
    assert_eq!(conf.idle_opacity_percent, 85);
}

#[test]
fn get_conf_path_under_app_dir() {
    let dir = OrbConf::get_conf_dir().unwrap();
    let path = OrbConf::get_conf_path().unwrap();
    assert!(dir.to_string_lossy().contains("com.assistant"));
    assert!(path.starts_with(&dir));
    assert!(path.to_string_lossy().ends_with("orb-config.json"));
}

#[test]
fn load_from_creates_default_when_missing() {
    let path = conf_path();
    let conf = OrbConf::load_from(&path).unwrap();
    assert_eq!(conf.scale_percent, 100);
    assert!(path.exists());
}

#[test]
fn save_to_and_load_from_roundtrip() {
    let path = conf_path();
    let mut conf = OrbConf::new();
    conf.scale_percent = 150;
    conf.opacity_percent = 80;
    conf.position_x = Some(100);
    conf.position_y = Some(200);

    conf.save_to(&path).unwrap();
    let loaded = OrbConf::load_from(&path).unwrap();

    assert_eq!(loaded.scale_percent, 150);
    assert_eq!(loaded.opacity_percent, 80);
    assert_eq!(loaded.position_x, Some(100));
    assert_eq!(loaded.position_y, Some(200));
}

#[test]
fn load_from_falls_back_on_corrupt_file() {
    let path = conf_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"broken json").unwrap();

    let loaded = OrbConf::load_from(&path).unwrap();
    assert_eq!(loaded.scale_percent, 100);
}

#[test]
fn load_from_recovers_from_temp_file() {
    let path = conf_path();
    let conf = OrbConf::new();
    conf.save_to(&path).unwrap();

    let temp = path.with_extension("tmp");
    let mut modified = conf.clone();
    modified.scale_percent = 200;
    let json = serde_json::to_string_pretty(&modified).unwrap();
    std::fs::write(&temp, json.as_bytes()).unwrap();

    let loaded = OrbConf::load_from(&path).unwrap();
    assert_eq!(loaded.scale_percent, 200);
    assert!(!temp.exists());
}
