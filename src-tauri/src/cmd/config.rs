use log::error;
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    AppConfDto, MascotConfDto, OrbConfDto, ThemeChangedPayloadDto, VaultConfDto,
};
use assistant_config::{
    app_conf::AppConf,
    constant::{WINDOW_MAIN, WINDOW_ORB},
    mascot_conf::MascotConf,
    orb_conf::OrbConf,
    vault_conf::VaultConf,
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
                    .set_always_on_top(crate::window::effective_on_top(conf.stay_on_top))
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
    ("theme", |app, prev, conf| {
        if prev.theme != conf.theme {
            let resolved = conf.theme.clone();
            emit_broadcast(
                app,
                event_name::THEME_CHANGED,
                ThemeChangedPayloadDto { resolved },
            );
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

pub(crate) fn apply_app_conf(app: &AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let conf = AppConf::load().map_err(|e| e.to_string())?;
    let prev = conf.clone();
    let merged = conf.amend(patch)?;
    merged.validate()?;
    merged.save().map_err(|e| e.to_string())?;
    apply_app_side_effects(app, &prev, &merged);
    Ok(())
}

/// 与 `patch_conf` 共用同一把配置锁，供窗口几何等非命令入口写入配置。
pub(crate) fn update_app_conf(app: &AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let _guard = CONF_LOCK.lock().map_err(|e| e.to_string())?;
    apply_app_conf(app, patch)
}

pub(crate) fn apply_orb_conf(app: &AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let conf = OrbConf::load().map_err(|e| e.to_string())?;
    let prev = conf.clone();
    let merged = conf.amend(patch)?;
    merged.validate()?;
    merged.save().map_err(|e| e.to_string())?;
    apply_orb_side_effects(app, &prev, &merged);
    Ok(())
}

pub(crate) fn apply_mascot_conf(_app: &AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let conf = MascotConf::load().map_err(|e| e.to_string())?;
    let merged = conf.amend(patch)?;
    merged.validate()?;
    merged.save().map_err(|e| e.to_string())?;
    Ok(())
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
pub fn get_mascot_conf() -> Result<MascotConfDto, String> {
    MascotConf::load().map(MascotConfDto::from).map_err(|e| {
        error!("Failed to load mascot config: {}", e);
        e.to_string()
    })
}

#[command]
pub fn get_vault_conf() -> Result<VaultConfDto, String> {
    VaultConf::load().map(VaultConfDto::from).map_err(|e| {
        error!("Failed to load vault config: {}", e);
        e.to_string()
    })
}

#[command]
pub fn patch_conf(app: AppHandle, group: String, patch: serde_json::Value) -> Result<(), String> {
    let _guard = CONF_LOCK.lock().map_err(|e| e.to_string())?;

    let (_, apply) = super::conf_side_effects()
        .into_iter()
        .find(|(name, _)| *name == group.as_str())
        .ok_or_else(|| format!("unknown config group: {}", group))?;

    apply(&app, patch)
}
