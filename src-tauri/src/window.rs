// 参考: ChatGPT/src-tauri/src/core/window.rs — 动态创建窗口
// 参考: BongoCat overlay placement — 窗口尺寸管理

use log::info;
use tauri::{command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use assistant_config::constant::{
    MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MIN_HEIGHT,
    MAIN_WINDOW_MIN_WIDTH, ORB_COLLAPSED_SIZE, ORB_EXPANDED_HEIGHT, ORB_EXPANDED_WIDTH,
    WINDOW_MAIN, WINDOW_ORB, WINDOW_SETTINGS,
};

#[command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_MAIN) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        info!("Main window already exists, brought to focus");
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, WINDOW_MAIN, WebviewUrl::App("/".into()))
        .title("Assistant")
        .inner_size(MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_DEFAULT_HEIGHT)
        .min_inner_size(MAIN_WINDOW_MIN_WIDTH, MAIN_WINDOW_MIN_HEIGHT)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    info!("Created main window");
    Ok(())
}

#[command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_SETTINGS) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, WINDOW_SETTINGS, WebviewUrl::App("/settings".into()))
        .title("Settings")
        .inner_size(600.0, 500.0)
        .center()
        .resizable(true)
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;

    info!("Created settings window");
    Ok(())
}

#[command]
pub fn expand_orb_panel(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_ORB) {
        window
            .set_size(tauri::PhysicalSize::new(
                ORB_EXPANDED_WIDTH,
                ORB_EXPANDED_HEIGHT,
            ))
            .map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        info!("Orb panel expanded");
    }
    Ok(())
}

#[command]
pub fn collapse_orb_panel(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_ORB) {
        window
            .set_size(tauri::PhysicalSize::new(
                ORB_COLLAPSED_SIZE,
                ORB_COLLAPSED_SIZE,
            ))
            .map_err(|e| e.to_string())?;
        info!("Orb panel collapsed");
    }
    Ok(())
}