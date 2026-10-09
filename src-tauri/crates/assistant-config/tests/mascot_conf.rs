use assistant_config::mascot_conf::MascotConf;

#[test]
fn new_returns_defaults() {
    let conf = MascotConf::new();
    assert!(conf.primary_color.is_none());
    assert!(conf.secondary_color.is_none());
    assert!(conf.validate().is_ok());
}

#[test]
fn validate_color_format() {
    let mut conf = MascotConf::new();
    conf.primary_color = Some("#ff0000".to_string());
    assert!(conf.validate().is_ok());

    conf.primary_color = Some("ff0000".to_string());
    assert!(conf.validate().is_err());

    conf.primary_color = Some("#fff".to_string());
    assert!(conf.validate().is_err());
}

#[test]
fn amend_single_field() {
    let conf = MascotConf::new();
    let patch = serde_json::json!({ "primary_color": "#00ff00" });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.primary_color, Some("#00ff00".to_string()));
    assert!(merged.secondary_color.is_none());
}

#[test]
fn amend_both_colors() {
    let conf = MascotConf::new();
    let patch = serde_json::json!({ "primary_color": "#ff0000", "secondary_color": "#0000ff" });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.primary_color, Some("#ff0000".to_string()));
    assert_eq!(merged.secondary_color, Some("#0000ff".to_string()));
}

#[test]
fn amend_reset_to_null() {
    let mut conf = MascotConf::new();
    conf.primary_color = Some("#ff0000".to_string());
    let patch = serde_json::json!({ "primary_color": null });
    let merged = conf.amend(patch).unwrap();
    assert!(merged.primary_color.is_none());
}

#[test]
fn amend_type_mismatch_returns_error() {
    let conf = MascotConf::new();
    let patch = serde_json::json!({ "primary_color": 123 });
    assert!(conf.amend(patch).is_err());
}
