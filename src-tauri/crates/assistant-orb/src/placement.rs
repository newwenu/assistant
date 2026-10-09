use std::time::Duration;

use log::error;
use tauri::{AppHandle, Manager};

use super::hover::ScreenBounds;

pub use super::geometry::{
    bounds_inside_screens, correction_for_screens, snap_orb_to_edge, snap_to_nearest_screen,
    WindowBounds,
};

pub const PLACEMENT_SETTLE_DELAY: Duration = Duration::from_millis(1000);
pub const PLACEMENT_INSPECTION_INTERVAL: Duration = Duration::from_millis(800);

pub fn get_available_screens(app: &AppHandle) -> Vec<ScreenBounds> {
    let mut screens = Vec::new();
    match app.available_monitors() {
        Ok(monitors) => {
            for monitor in monitors {
                let pos = monitor.position();
                let size = monitor.size();
                screens.push(ScreenBounds {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
        Err(e) => {
            error!("Failed to get available monitors: {}", e);
        }
    }
    if screens.is_empty() {
        if let Ok(primary) = app.primary_monitor() {
            if let Some(monitor) = primary {
                let pos = monitor.position();
                let size = monitor.size();
                screens.push(ScreenBounds {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
    }
    screens
}

pub fn check_and_snap_orb(app: &AppHandle, window_label: &str) -> Option<WindowBounds> {
    let window = app.get_webview_window(window_label)?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let bounds = WindowBounds::new(pos.x, pos.y, size.width, size.height);
    let screens = get_available_screens(app);
    if bounds_inside_screens(&screens, bounds) {
        return None;
    }
    let snapped = snap_to_nearest_screen(&screens, bounds);
    let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
    Some(snapped)
}
