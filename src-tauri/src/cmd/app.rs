use tauri::{command, AppHandle};

use crate::protocol::events::{emit_broadcast, event_name};

use assistant_config::{
    constant::{WINDOW_MAIN, WINDOW_ORB},
    orb_conf::OrbConf,
};

#[command]
pub fn app_quit(app: AppHandle) -> Result<(), String> {
    perform_exit(&app);
    Ok(())
}

pub fn perform_exit(app: &AppHandle) {
    if let Ok(w) = crate::window::get_or_error(app, WINDOW_ORB) {
        if let Ok(pos) = w.outer_position() {
            if let Ok(mut conf) = OrbConf::load() {
                conf.position_x = Some(pos.x);
                conf.position_y = Some(pos.y);
                let _ = conf.save();
            }
        }
    }
    crate::lifecycle::request_exit();
    crate::window::destroy_all(app, &[WINDOW_ORB, WINDOW_MAIN]);
    app.exit(0);
}

#[command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[command]
pub fn get_rustc_version() -> String {
    rustc_version_runtime::version().to_string()
}

#[command]
pub fn open_main_window_and_navigate(app: AppHandle, route: String) -> Result<(), String> {
    crate::window::show_or_create(&app, WINDOW_MAIN)?;
    emit_broadcast(&app, event_name::NAVIGATE, route);
    Ok(())
}
