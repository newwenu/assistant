use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use log::error;
use mouse_position::mouse_position::Mouse;
use serde_json::json;
use tauri::{command, AppHandle, PhysicalPosition};

use assistant_config::{
    constant::{
        ORB_CHROME_INSET, ORB_HITBOX_SIZE, ORB_SNAP_ANIMATION_MS, ORB_SNAP_BORDER_X,
        WINDOW_EDGE_BUFFER, WINDOW_ORB,
    },
    orb_conf::OrbConf,
};
use assistant_orb::placement;

pub(crate) static ORB_CLICK_THROUGH: AtomicBool = AtomicBool::new(false);
static ORB_SNAP_CANCEL: AtomicBool = AtomicBool::new(false);

#[command]
pub fn get_mouse_position() -> serde_json::Value {
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => json!({
            "clientX": x,
            "clientY": y
        }),
        Mouse::Error => {
            error!("Error getting mouse position");
            json!(null)
        }
    }
}

#[command]
pub fn toggle_orb_visibility(app: AppHandle) -> Result<(), String> {
    crate::window::toggle(&app, WINDOW_ORB)
}

#[command]
pub fn set_orb_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    if ORB_CLICK_THROUGH.load(Ordering::Acquire) == enabled {
        return Ok(());
    }
    let window = crate::window::get_or_error(&app, WINDOW_ORB)?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| e.to_string())?;
    ORB_CLICK_THROUGH.store(enabled, Ordering::Release);
    log::debug!("Orb click-through set to {}", enabled);
    Ok(())
}

pub(crate) fn apply_orb_click_through(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if ORB_CLICK_THROUGH.load(Ordering::Acquire) == enabled {
        return Ok(());
    }
    let window = crate::window::get_or_error(app, WINDOW_ORB)?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| e.to_string())?;
    ORB_CLICK_THROUGH.store(enabled, Ordering::Release);
    Ok(())
}

#[command]
pub fn get_screen_size(
    app: AppHandle,
    center_x: f64,
    center_y: f64,
    scale_factor: f64,
) -> serde_json::Value {
    let cx_phys = center_x * scale_factor;
    let cy_phys = center_y * scale_factor;
    let screens = placement::get_available_screens(&app);
    let best = screens.iter().min_by_key(|s| {
        let sx = s.x + s.width as i32 / 2;
        let sy = s.y + s.height as i32 / 2;
        let dx = cx_phys as i64 - sx as i64;
        let dy = cy_phys as i64 - sy as i64;
        dx * dx + dy * dy
    });
    match best {
        Some(s) => json!({
            "screen": { "x": s.x, "y": s.y, "width": s.width, "height": s.height }
        }),
        None => json!({ "screen": { "x": 0, "y": 0, "width": 1920, "height": 1080 } }),
    }
}

#[command]
pub fn snap_orb_to_screen(app: AppHandle) -> Result<(), String> {
    let window = crate::window::get_or_error(&app, WINDOW_ORB)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    let screens = placement::get_available_screens(&app);
    let Some(target) = placement::ensure_orb_on_screen(
        &screens,
        pos.x,
        pos.y,
        size.width,
        size.height,
        ORB_CHROME_INSET,
        ORB_HITBOX_SIZE,
    ) else {
        let _ = save_orb_position_sync(&app);
        return Ok(());
    };

    if target.x == pos.x && target.y == pos.y {
        let _ = save_orb_position_sync(&app);
        return Ok(());
    }

    let _ = window.set_position(PhysicalPosition::new(target.x, target.y));
    let _ = save_orb_position_sync(&app);
    Ok(())
}

#[command]
pub fn snap_orb_to_edge(app: AppHandle) -> Result<(), String> {
    ORB_SNAP_CANCEL.store(true, Ordering::Release);

    let window = crate::window::get_or_error(&app, WINDOW_ORB)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    let screens = placement::get_available_screens(&app);
    let bounds = placement::WindowBounds::new(pos.x, pos.y, size.width, size.height);
    let border_x = ORB_SNAP_BORDER_X;
    let target = placement::snap_orb_to_edge(&screens, bounds, WINDOW_EDGE_BUFFER as i32, border_x);

    if target.x == bounds.x && target.y == bounds.y {
        let _ = save_orb_position_sync(&app);
        return Ok(());
    }

    ORB_SNAP_CANCEL.store(false, Ordering::Release);

    let start_x = bounds.x as f64;
    let start_y = bounds.y as f64;
    let end_x = target.x as f64;
    let end_y = target.y as f64;
    let duration = Duration::from_millis(ORB_SNAP_ANIMATION_MS);
    let step = Duration::from_millis(16);

    std::thread::spawn(move || {
        let start = Instant::now();
        let mut cancelled = false;
        loop {
            if ORB_SNAP_CANCEL.load(Ordering::Acquire) {
                cancelled = true;
                break;
            }
            std::thread::sleep(step);
            let elapsed = start.elapsed();
            if elapsed >= duration {
                let _ = window.set_position(PhysicalPosition::new(target.x, target.y));
                break;
            }
            let dur_secs = duration.as_secs_f64();
            let t = if dur_secs > 0.0 {
                elapsed.as_secs_f64() / dur_secs
            } else {
                1.0
            };
            let t = 1.0 - (1.0 - t).powi(3);
            let x = (start_x + (end_x - start_x) * t).round() as i32;
            let y = (start_y + (end_y - start_y) * t).round() as i32;
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        if !cancelled {
            if let Ok(pos) = window.outer_position() {
                if let Ok(mut conf) = OrbConf::load() {
                    conf.position_x = Some(pos.x);
                    conf.position_y = Some(pos.y);
                    let _ = conf.save();
                }
            }
        }
    });

    Ok(())
}

fn save_orb_position_sync(app: &AppHandle) -> Result<(), String> {
    let window = crate::window::get_or_error(app, WINDOW_ORB)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut conf = OrbConf::load().map_err(|e| e.to_string())?;
    conf.position_x = Some(pos.x);
    conf.position_y = Some(pos.y);
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn save_orb_position(app: AppHandle) -> Result<(), String> {
    let window = crate::window::get_or_error(&app, WINDOW_ORB)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut conf = OrbConf::load().map_err(|e| e.to_string())?;
    conf.position_x = Some(pos.x);
    conf.position_y = Some(pos.y);
    conf.save().map_err(|e| e.to_string())
}