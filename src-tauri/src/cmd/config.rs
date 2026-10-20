use log::error;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{AppConfDto, MascotConfDto, OrbConfDto, ThemeChangedPayloadDto};
use assistant_config::{
    app_conf::AppConf,
    atomic_write::atomic_write,
    constant::{WINDOW_MAIN, WINDOW_ORB},
    mascot_conf::MascotConf,
    orb_conf::OrbConf,
};

static CONF_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Side-effect registry
// ---------------------------------------------------------------------------

type AppSideEffectFn = fn(&AppHandle, &AppConf, &AppConf) -> Result<(), String>;
type OrbSideEffectFn = fn(&AppHandle, &OrbConf, &OrbConf) -> Result<(), String>;

static APP_SIDE_EFFECTS: &[(&str, AppSideEffectFn)] = &[
    ("launch_at_login", |_app, prev, conf| {
        if prev.launch_at_login != conf.launch_at_login {
            apply_autostart(_app, conf.launch_at_login)?;
        }
        Ok(())
    }),
    ("stay_on_top", |_app, prev, conf| {
        if prev.stay_on_top != conf.stay_on_top {
            if let Some(window) = _app.get_webview_window(WINDOW_MAIN) {
                window
                    .set_always_on_top(conf.stay_on_top)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }),
    ("keybindings", |app, prev, conf| {
        if prev.keybindings != conf.keybindings {
            crate::setup::register_global_shortcuts(app);
        }
        Ok(())
    }),
    ("language", |app, prev, conf| {
        if prev.language != conf.language {
            assistant_i18n::set_locale(&conf.language);
            crate::tray::rebuild_tray(app);
        }
        Ok(())
    }),
];

static ORB_SIDE_EFFECTS: &[(&str, OrbSideEffectFn)] = &[
    ("always_on_top", |_app, prev, conf| {
        if prev.always_on_top != conf.always_on_top {
            if let Some(window) = _app.get_webview_window(WINDOW_ORB) {
                window
                    .set_always_on_top(conf.always_on_top)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }),
    ("click_through", |_app, prev, conf| {
        if prev.click_through != conf.click_through {
            super::orb::apply_orb_click_through(_app, conf.click_through)?;
        }
        Ok(())
    }),
];

fn apply_app_side_effects(app: &AppHandle, prev: &AppConf, conf: &AppConf) {
    for (name, handler) in APP_SIDE_EFFECTS.iter() {
        if let Err(e) = handler(app, prev, conf) {
            log::error!("[side_effect:app:{}] {}", name, e);
        }
    }
}

fn apply_orb_side_effects(app: &AppHandle, prev: &OrbConf, conf: &OrbConf) {
    for (name, handler) in ORB_SIDE_EFFECTS.iter() {
        if let Err(e) = handler(app, prev, conf) {
            log::error!("[side_effect:orb:{}] {}", name, e);
        }
    }
}

fn apply_autostart(app: &AppHandle, enable: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enable {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[command]
pub fn get_app_conf() -> Result<AppConfDto, String> {
    AppConf::load().map(AppConfDto::from).map_err(|e| {
        error!("Failed to load app config: {}", e);
        e.to_string()
    })
}

#[command]
pub fn get_orb_conf() -> Result<OrbConfDto, String> {
    OrbConf::load().map(OrbConfDto::from).map_err(|e| {
        error!("Failed to load orb config: {}", e);
        e.to_string()
    })
}

#[command]
pub fn set_theme(theme: String) -> Result<(), String> {
    let mut conf = AppConf::load().map_err(|e| e.to_string())?;
    conf.theme = theme;
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn emit_theme_changed(app: AppHandle, resolved: String) -> Result<(), String> {
    emit_broadcast(
        &app,
        event_name::THEME_CHANGED,
        ThemeChangedPayloadDto { resolved },
    );
    Ok(())
}

#[command]
pub fn set_locale(app: AppHandle, locale: String) -> Result<(), String> {
    assistant_i18n::set_locale(&locale);
    crate::tray::rebuild_tray(&app);
    Ok(())
}

#[command]
pub fn get_mascot_conf() -> Result<MascotConfDto, String> {
    MascotConf::load().map(MascotConfDto::from).map_err(|e| {
        error!("Failed to load mascot config: {}", e);
        e.to_string()
    })
}

#[command]
pub fn patch_conf(app: AppHandle, group: String, patch: serde_json::Value) -> Result<(), String> {
    let _guard = CONF_LOCK.lock().map_err(|e| e.to_string())?;

    match group.as_str() {
        "app" => {
            let conf = AppConf::load().map_err(|e| e.to_string())?;
            let prev = conf.clone();
            let merged = conf.amend(patch)?;
            merged.validate()?;
            merged.save().map_err(|e| e.to_string())?;
            apply_app_side_effects(&app, &prev, &merged);
        }
        "orb" => {
            let conf = OrbConf::load().map_err(|e| e.to_string())?;
            let prev = conf.clone();
            let merged = conf.amend(patch)?;
            merged.validate()?;
            merged.save().map_err(|e| e.to_string())?;
            apply_orb_side_effects(&app, &prev, &merged);
        }
        "mascot" => {
            let conf = MascotConf::load().map_err(|e| e.to_string())?;
            let merged = conf.amend(patch)?;
            merged.validate()?;
            merged.save().map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("unknown config group: {}", group)),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Mascot asset install / uninstall
// ---------------------------------------------------------------------------

const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024;

struct AssetSpec {
    dir_name: &'static str,
    allowed_extensions: &'static [&'static str],
    target_filename: &'static str,
}

fn get_asset_spec(asset_type: &str) -> Result<AssetSpec, String> {
    match asset_type {
        "rive" => Ok(AssetSpec {
            dir_name: "rive",
            allowed_extensions: &[".riv"],
            target_filename: "custom",
        }),
        "gif" => Ok(AssetSpec {
            dir_name: "gif",
            allowed_extensions: &[".gif", ".png", ".webp"],
            target_filename: "custom",
        }),
        "svg" => Ok(AssetSpec {
            dir_name: "svg",
            allowed_extensions: &[".svg"],
            target_filename: "custom",
        }),
        _ => Err(format!("unknown asset_type: {}", asset_type)),
    }
}

fn mascot_assets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(data_dir.join("mascot-assets"))
}

fn validate_mime(data: &[u8], asset_type: &str) -> Result<(), String> {
    match asset_type {
        "gif" => {
            if data.len() < 8 {
                return Err("file too small to identify".to_string());
            }
            let is_gif = &data[0..3] == b"GIF";
            let is_png = &data[0..8] == b"\x89PNG\r\n\x1a\n";
            let is_webp = data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP";
            if !is_gif && !is_png && !is_webp {
                return Err("not a valid image file (expected GIF, PNG, or WebP)".to_string());
            }
        }
        "svg" => {
            let head_len = data.len().min(256);
            let head_str = String::from_utf8_lossy(&data[..head_len]).to_lowercase();
            if !head_str.contains("<svg") {
                return Err("not a valid SVG file".to_string());
            }
        }
        "rive" => {
            if data.len() < 4 || &data[0..4] != b"RIVE" {
                return Err("not a valid Rive runtime file (missing RIVE header)".to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Serialize)]
pub struct InstallMascotAssetResult {
    pub installed_path: String,
    pub bytes: u64,
    pub version: u64,
}

#[command]
pub async fn install_mascot_asset(
    app: AppHandle,
    asset_type: String,
    file_path: String,
) -> Result<InstallMascotAssetResult, String> {
    let spec = get_asset_spec(&asset_type)?;

    let src_path = PathBuf::from(&file_path);
    if !src_path.exists() {
        return Err(format!("source file not found: {}", file_path));
    }

    let src_ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let ext_with_dot = format!(".{}", src_ext);
    if !spec.allowed_extensions.contains(&ext_with_dot.as_str()) {
        return Err(format!(
            "extension .{} not allowed for asset_type \"{}\"; allowed: {:?}",
            src_ext, asset_type, spec.allowed_extensions
        ));
    }

    let data = fs::read(&src_path).map_err(|e| format!("read failed: {}", e))?;

    if data.len() as u64 > MAX_ASSET_BYTES {
        return Err(format!(
            "file too large: {} bytes (max {} bytes)",
            data.len(),
            MAX_ASSET_BYTES
        ));
    }

    validate_mime(&data, &asset_type)?;

    let assets_dir = mascot_assets_dir(&app)?;
    let target_dir = assets_dir.join(spec.dir_name);
    let target_name = format!("{}.{}", spec.target_filename, src_ext);
    let target_path = target_dir.join(&target_name);

    atomic_write(&target_path, &data).map_err(|e| format!("write failed: {}", e))?;

    let metadata = fs::metadata(&target_path).map_err(|e| format!("metadata failed: {}", e))?;
    let version = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(InstallMascotAssetResult {
        installed_path: target_path.to_string_lossy().to_string(),
        bytes: data.len() as u64,
        version,
    })
}

#[command]
pub async fn uninstall_mascot_asset(app: AppHandle, asset_type: String) -> Result<(), String> {
    let spec = get_asset_spec(&asset_type)?;
    let target_dir = mascot_assets_dir(&app)?.join(spec.dir_name);
    for ext in spec.allowed_extensions {
        let path = target_dir.join(format!("{}{}", spec.target_filename, ext));
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
    if asset_type == "rive" {
        let manifest_path = mascot_assets_dir(&app)?.join("rive/custom-manifest.json");
        if manifest_path.exists() {
            let _ = fs::remove_file(&manifest_path);
        }
    }
    Ok(())
}
