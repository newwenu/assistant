// 参考: docs/store/I75-startup-layering-spec.md §5 — Orb 面板尺寸与右键菜单

use log::info;
use tauri::{
    command,
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem},
    AppHandle,
};

use assistant_config::constant::{
    ORB_CHROME_INSET, ORB_COLLAPSED_SIZE, ORB_COLLAPSED_WINDOW_SIZE, ORB_EXPANDED_HEIGHT,
    ORB_EXPANDED_WIDTH, ORB_EXPANDED_WINDOW_HEIGHT, ORB_EXPANDED_WINDOW_WIDTH, WINDOW_ORB,
};

use super::create::get_or_error;

#[command]
pub fn expand_orb_panel(
    app: AppHandle,
    horizontal: String,
    vertical: String,
) -> Result<(), String> {
    if crate::screenshot::is_active() {
        return Ok(());
    }
    let window = get_or_error(&app, WINDOW_ORB)?;

    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if scale <= 0.0 {
        return Err("scale_factor returned non-positive value".to_string());
    }
    let old_pos = window.outer_position().map_err(|e| e.to_string())?;
    let old_size = window.inner_size().map_err(|e| e.to_string())?;

    let old_logical_w = old_size.width as f64 / scale;
    let old_logical_h = old_size.height as f64 / scale;

    let ball_x = if horizontal == "left" {
        old_pos.x as f64 / scale + old_logical_w
            - ORB_CHROME_INSET as f64
            - ORB_COLLAPSED_SIZE as f64
    } else {
        old_pos.x as f64 / scale + ORB_CHROME_INSET as f64
    };
    let ball_y = if vertical == "up" {
        old_pos.y as f64 / scale + old_logical_h
            - ORB_CHROME_INSET as f64
            - ORB_COLLAPSED_SIZE as f64
    } else {
        old_pos.y as f64 / scale + ORB_CHROME_INSET as f64
    };

    let new_x = if horizontal == "left" {
        ball_x - (ORB_EXPANDED_WIDTH as f64 - ORB_COLLAPSED_SIZE as f64) - ORB_CHROME_INSET as f64
    } else {
        ball_x - ORB_CHROME_INSET as f64
    };
    let new_y = if vertical == "up" {
        ball_y - (ORB_EXPANDED_HEIGHT as f64 - ORB_COLLAPSED_SIZE as f64) - ORB_CHROME_INSET as f64
    } else {
        ball_y - ORB_CHROME_INSET as f64
    };

    let phys_x = (new_x * scale).round() as i32;
    let phys_y = (new_y * scale).round() as i32;

    window
        .set_position(tauri::PhysicalPosition::new(phys_x, phys_y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(
            ORB_EXPANDED_WINDOW_WIDTH as f64,
            ORB_EXPANDED_WINDOW_HEIGHT as f64,
        ))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    info!("Orb panel expanded ({} {})", horizontal, vertical);
    Ok(())
}

#[command]
pub fn collapse_orb_panel(
    app: AppHandle,
    horizontal: String,
    vertical: String,
) -> Result<(), String> {
    if crate::screenshot::is_active() {
        return Ok(());
    }
    let window = get_or_error(&app, WINDOW_ORB)?;

    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if scale <= 0.0 {
        return Err("scale_factor returned non-positive value".to_string());
    }
    let old_pos = window.outer_position().map_err(|e| e.to_string())?;
    let old_size = window.inner_size().map_err(|e| e.to_string())?;

    let old_logical_w = old_size.width as f64 / scale;
    let old_logical_h = old_size.height as f64 / scale;

    let ball_x = if horizontal == "left" {
        old_pos.x as f64 / scale + old_logical_w
            - ORB_CHROME_INSET as f64
            - ORB_COLLAPSED_SIZE as f64
    } else {
        old_pos.x as f64 / scale + ORB_CHROME_INSET as f64
    };
    let ball_y = if vertical == "up" {
        old_pos.y as f64 / scale + old_logical_h
            - ORB_CHROME_INSET as f64
            - ORB_COLLAPSED_SIZE as f64
    } else {
        old_pos.y as f64 / scale + ORB_CHROME_INSET as f64
    };

    let new_x = ball_x - ORB_CHROME_INSET as f64;
    let new_y = ball_y - ORB_CHROME_INSET as f64;

    let phys_x = (new_x * scale).round() as i32;
    let phys_y = (new_y * scale).round() as i32;

    window
        .set_position(tauri::PhysicalPosition::new(phys_x, phys_y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(
            ORB_COLLAPSED_WINDOW_SIZE as f64,
            ORB_COLLAPSED_WINDOW_SIZE as f64,
        ))
        .map_err(|e| e.to_string())?;
    info!("Orb panel collapsed ({} {})", horizontal, vertical);
    Ok(())
}

#[command]
pub fn show_orb_context_menu(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    // 截图期间禁止再弹菜单：抓屏时新弹出的菜单会直接进入底图。
    if crate::screenshot::is_active() {
        return Ok(());
    }

    let new_chat = MenuItem::with_id(
        &app,
        "orb_new_chat",
        assistant_i18n::t("orb.context_new_chat"),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let open_main = MenuItem::with_id(
        &app,
        "orb_open_main",
        assistant_i18n::t("orb.context_open_main"),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let screenshot = MenuItem::with_id(
        &app,
        "orb_screenshot",
        assistant_i18n::t("orb.context_screenshot"),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(
        &app,
        "orb_quit",
        assistant_i18n::t("orb.context_quit"),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        &app,
        &[&new_chat, &open_main, &screenshot, &separator, &quit],
    )
    .map_err(|e| e.to_string())?;

    let webview_window = get_or_error(&app, WINDOW_ORB)?;
    let window = webview_window.as_ref().window();

    // 菜单是独立顶层窗口（Orb 只是 owner），点中菜单项后系统会播放关闭淡出，这段时间窗口仍在合成。
    // 截图项的淡出起点已在菜单事件回调（点击那一刻）锚定；这里只做兜底：把仍在关闭动画中的
    // 菜单窗口立刻隐藏，避免残留菜单继续被合成进后续抓屏。
    let result = menu.popup_at(window, tauri::LogicalPosition::new(x, y));
    crate::screenshot::orb_overlay::dismiss_menus_now();

    result.map_err(|e: tauri::Error| e.to_string())
}
