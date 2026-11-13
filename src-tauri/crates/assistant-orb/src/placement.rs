use std::time::Duration;

use log::error;
use tauri::AppHandle;

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
        #[allow(clippy::collapsible_match)]
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

/// 可用区右下角锚点（I47 §7.2）：在 `area` 内按 `margin` 内缩放置边长为 `size` 的方形。
pub fn anchor_bottom_right(area: &ScreenBounds, size: u32, margin: i32) -> (i32, i32) {
    let x = area.x + area.width as i32 - size as i32 - margin;
    let y = area.y + area.height as i32 - size as i32 - margin;
    (x, y)
}

/// 主显示器可用区（I47 §7.2）：排除任务栏/程序坞。
///
/// 冷启动锚点用它而非整屏，否则边距小于任务栏高度时球会落进任务栏。
pub fn get_primary_work_area(app: &AppHandle) -> Option<ScreenBounds> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let area = monitor.work_area();
    Some(ScreenBounds {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    })
}

pub fn ensure_orb_on_screen(
    screens: &[ScreenBounds],
    win_x: i32,
    win_y: i32,
    win_w: u32,
    win_h: u32,
    chrome_inset: u32,
    hitbox_size: u32,
) -> Option<WindowBounds> {
    let visual_x = win_x + chrome_inset as i32;
    let visual_y = win_y + chrome_inset as i32;
    let visual_bounds = WindowBounds::new(visual_x, visual_y, hitbox_size, hitbox_size);
    if bounds_inside_screens(screens, visual_bounds) {
        return None;
    }
    let bounds = WindowBounds::new(win_x, win_y, win_w, win_h);
    Some(snap_to_nearest_screen(screens, bounds))
}
