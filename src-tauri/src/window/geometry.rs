// 参考: docs/store/I75-startup-layering-spec.md §5 — 主窗口几何与形态切换的落盘

use tauri::{AppHandle, Manager};

use assistant_config::app_conf::AppConf;
use assistant_config::constant::{
    MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MINI_HEIGHT,
    MAIN_WINDOW_MINI_MIN_HEIGHT, MAIN_WINDOW_MINI_MIN_WIDTH, MAIN_WINDOW_MINI_WIDTH,
    MAIN_WINDOW_MIN_HEIGHT, MAIN_WINDOW_MIN_WIDTH, WINDOW_MAIN,
};
use assistant_orb::placement::{self, WindowBounds};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::WindowModeChangedPayloadDto;

use super::create::get_or_error;
use super::mode::{current_mode, set_mode, MainWindowMode};

fn logical_scale(window: &tauri::WebviewWindow) -> Result<f64, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if scale <= 0.0 {
        return Err("scale_factor returned non-positive value".to_string());
    }
    Ok(scale)
}

pub(super) fn saved_normal_size(conf: &AppConf) -> (f64, f64) {
    let width = conf
        .main_window_width
        .filter(|w| f64::from(*w) >= MAIN_WINDOW_MIN_WIDTH)
        .map(f64::from)
        .unwrap_or(MAIN_WINDOW_DEFAULT_WIDTH);
    let height = conf
        .main_window_height
        .filter(|h| f64::from(*h) >= MAIN_WINDOW_MIN_HEIGHT)
        .map(f64::from)
        .unwrap_or(MAIN_WINDOW_DEFAULT_HEIGHT);
    (width, height)
}

pub(super) fn snap_position(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let screens = placement::get_available_screens(app);
    let bounds = placement::snap_to_nearest_screen(
        &screens,
        WindowBounds::new(x, y, size.width, size.height),
    );
    window
        .set_position(tauri::PhysicalPosition::new(bounds.x, bounds.y))
        .map_err(|e| e.to_string())
}

fn apply_mini(window: &tauri::WebviewWindow, conf: &AppConf) -> Result<(), String> {
    window.set_resizable(false).map_err(|e| e.to_string())?;
    window
        .set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
            MAIN_WINDOW_MINI_MIN_WIDTH,
            MAIN_WINDOW_MINI_MIN_HEIGHT,
        ))))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(
            MAIN_WINDOW_MINI_WIDTH,
            MAIN_WINDOW_MINI_HEIGHT,
        ))
        .map_err(|e| e.to_string())?;

    let current = window.outer_position().map_err(|e| e.to_string())?;
    let (x, y) = match (conf.mini_window_x, conf.mini_window_y) {
        (Some(x), Some(y)) => (x, y),
        _ => (current.x, current.y),
    };
    snap_position(window.app_handle(), window, x, y)?;
    window.set_always_on_top(true).map_err(|e| e.to_string())
}

fn apply_normal(window: &tauri::WebviewWindow, conf: &AppConf) -> Result<(), String> {
    let (width, height) = saved_normal_size(conf);
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
            MAIN_WINDOW_MIN_WIDTH,
            MAIN_WINDOW_MIN_HEIGHT,
        ))))
        .map_err(|e| e.to_string())?;
    window.set_resizable(true).map_err(|e| e.to_string())?;

    let current = window.outer_position().map_err(|e| e.to_string())?;
    let (x, y) = match (conf.main_window_x, conf.main_window_y) {
        (Some(x), Some(y)) => (x, y),
        _ => (current.x, current.y),
    };
    snap_position(window.app_handle(), window, x, y)?;
    window
        .set_always_on_top(conf.stay_on_top)
        .map_err(|e| e.to_string())
}

/// 把当前形态的几何写入 AppConf。normal 存物理位置 + 逻辑尺寸，mini 只存物理位置。
pub fn persist_main_geometry(app: &AppHandle) -> Result<(), String> {
    let window = get_or_error(app, WINDOW_MAIN)?;
    let position = window.outer_position().map_err(|e| e.to_string())?;

    let patch = match current_mode() {
        MainWindowMode::Mini => serde_json::json!({
            "mini_window_x": position.x,
            "mini_window_y": position.y,
        }),
        MainWindowMode::Normal => {
            let size = window.inner_size().map_err(|e| e.to_string())?;
            let scale = logical_scale(&window)?;
            let logical_width = (f64::from(size.width) / scale).round().max(0.0) as u32;
            let logical_height = (f64::from(size.height) / scale).round().max(0.0) as u32;
            serde_json::json!({
                "main_window_x": position.x,
                "main_window_y": position.y,
                "main_window_width": logical_width,
                "main_window_height": logical_height,
            })
        }
    };

    crate::cmd::update_app_conf(app, patch)
}

/// 形态切换入口：先落盘旧几何，再按目标形态重排，最后广播。
pub fn apply_mode(app: &AppHandle, target: MainWindowMode) -> Result<(), String> {
    if current_mode() == target {
        return Ok(());
    }

    persist_main_geometry(app)?;
    let window = get_or_error(app, WINDOW_MAIN)?;
    let conf = AppConf::load().map_err(|e| e.to_string())?;

    match target {
        MainWindowMode::Mini => apply_mini(&window, &conf)?,
        MainWindowMode::Normal => apply_normal(&window, &conf)?,
    }
    set_mode(target);
    emit_broadcast(
        app,
        event_name::WINDOW_MODE_CHANGED,
        WindowModeChangedPayloadDto {
            mode: target.as_dto(),
        },
    );
    Ok(())
}
