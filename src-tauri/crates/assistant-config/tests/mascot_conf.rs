use assistant_config::mascot_conf::MascotConf;

#[test]
fn new_returns_defaults() {
    let conf = MascotConf::new();
    assert_eq!(conf.renderer, "svg");
    assert!(conf.primary_color.is_none());
    assert!(conf.secondary_color.is_none());
    assert!(!conf.rive_idle_pose_rotation);
    assert!(conf.rive_src.is_none());
    assert!(conf.svg_animated);
    assert!(conf.gif_src.is_none());
    assert_eq!(conf.gif_border_radius, 50);
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
fn validate_renderer_must_be_known() {
    let mut conf = MascotConf::new();
    for renderer in &["rive", "svg", "gif", "disabled"] {
        conf.renderer = renderer.to_string();
        assert!(conf.validate().is_ok());
    }
    conf.renderer = "unknown".to_string();
    assert!(conf.validate().is_err());
}

#[test]
fn validate_rive_src_rejects_empty() {
    let mut conf = MascotConf::new();
    conf.rive_src = Some("".to_string());
    assert!(conf.validate().is_err());
    conf.rive_src = Some("mascot.riv".to_string());
    assert!(conf.validate().is_ok());
}

#[test]
fn validate_gif_src_rejects_empty() {
    let mut conf = MascotConf::new();
    conf.gif_src = Some("".to_string());
    assert!(conf.validate().is_err());
    conf.gif_src = Some("mascot.gif".to_string());
    assert!(conf.validate().is_ok());
}

#[test]
fn validate_gif_border_radius_bounds() {
    let mut conf = MascotConf::new();
    conf.gif_border_radius = 50;
    assert!(conf.validate().is_ok());
    conf.gif_border_radius = 51;
    assert!(conf.validate().is_err());
    conf.gif_border_radius = 0;
    assert!(conf.validate().is_ok());
}

#[test]
fn serialize_deserialize_roundtrip() {
    let original = MascotConf::new();
    let json = serde_json::to_string(&original).unwrap();
    let restored: MascotConf = serde_json::from_str(&json).unwrap();

    assert_eq!(original.renderer, restored.renderer);
    assert_eq!(original.primary_color, restored.primary_color);
    assert_eq!(original.secondary_color, restored.secondary_color);
    assert_eq!(original.svg_animated, restored.svg_animated);
    assert_eq!(original.gif_border_radius, restored.gif_border_radius);
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

#[test]
fn amend_renderer_and_rive_fields() {
    let conf = MascotConf::new();
    let patch = serde_json::json!({
        "renderer": "svg",
        "svg_animated": false,
        "rive_idle_pose_rotation": true,
        "rive_src": "custom.riv"
    });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.renderer, "svg");
    assert!(!merged.svg_animated);
    assert!(merged.rive_idle_pose_rotation);
    assert_eq!(merged.rive_src.as_deref(), Some("custom.riv"));
}

#[test]
fn amend_gif_fields() {
    let conf = MascotConf::new();
    let patch = serde_json::json!({
        "renderer": "gif",
        "gif_src": "mascot.gif",
        "gif_border_radius": 20
    });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.renderer, "gif");
    assert_eq!(merged.gif_src.as_deref(), Some("mascot.gif"));
    assert_eq!(merged.gif_border_radius, 20);
}
