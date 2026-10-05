// 参考: ChatGPT/src-tauri/src/core/window.rs — 动态创建窗口
// 参考: BongoCat overlay placement — 窗口尺寸管理

use std::sync::Mutex;

use log::info;
use tauri::{
    command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem},
};

use assistant_config::constant::{
    MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MIN_HEIGHT,
    MAIN_WINDOW_MIN_WIDTH, ORB_COLLAPSED_SIZE, ORB_EXPANDED_HEIGHT, ORB_EXPANDED_WIDTH,
    ORB_CHROME_INSET, ORB_COLLAPSED_WINDOW_SIZE, ORB_EXPANDED_WINDOW_WIDTH,
    ORB_EXPANDED_WINDOW_HEIGHT, WINDOW_MAIN, WINDOW_ORB, WINDOW_SETTINGS,
};

static MAIN_LOCK: Mutex<()> = Mutex::new(());

pub fn show_or_create_main(app: &AppHandle) {
    let _lock = MAIN_LOCK.lock().unwrap();
    if let Some(window) = app.get_webview_window(WINDOW_MAIN) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let result = WebviewWindowBuilder::new(app, WINDOW_MAIN, WebviewUrl::App("/".into()))
        .title("Assistant")
        .inner_size(MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_DEFAULT_HEIGHT)
        .min_inner_size(MAIN_WINDOW_MIN_WIDTH, MAIN_WINDOW_MIN_HEIGHT)
        .center()
        .build();
    if result.is_err() {
        if let Some(window) = app.get_webview_window(WINDOW_MAIN) {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

#[command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    show_or_create_main(&app);
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
pub fn expand_orb_panel(
    app: AppHandle,
    horizontal: String,
    vertical: String,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_ORB) {
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let old_pos = window.outer_position().map_err(|e| e.to_string())?;
        let old_size = window.inner_size().map_err(|e| e.to_string())?;

        let old_logical_w = old_size.width as f64 / scale;
        let old_logical_h = old_size.height as f64 / scale;

        let ball_x = if horizontal == "left" {
            old_pos.x as f64 / scale + old_logical_w - ORB_CHROME_INSET as f64 - ORB_COLLAPSED_SIZE as f64
        } else {
            old_pos.x as f64 / scale + ORB_CHROME_INSET as f64
        };
        let ball_y = if vertical == "up" {
            old_pos.y as f64 / scale + old_logical_h - ORB_CHROME_INSET as f64 - ORB_COLLAPSED_SIZE as f64
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
    }
    Ok(())
}

#[command]
pub fn collapse_orb_panel(
    app: AppHandle,
    horizontal: String,
    vertical: String,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(WINDOW_ORB) {
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let old_pos = window.outer_position().map_err(|e| e.to_string())?;
        let old_size = window.inner_size().map_err(|e| e.to_string())?;

        let old_logical_w = old_size.width as f64 / scale;
        let old_logical_h = old_size.height as f64 / scale;

        let ball_x = if horizontal == "left" {
            old_pos.x as f64 / scale + old_logical_w - ORB_CHROME_INSET as f64 - ORB_COLLAPSED_SIZE as f64
        } else {
            old_pos.x as f64 / scale + ORB_CHROME_INSET as f64
        };
        let ball_y = if vertical == "up" {
            old_pos.y as f64 / scale + old_logical_h - ORB_CHROME_INSET as f64 - ORB_COLLAPSED_SIZE as f64
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
    }
    Ok(())
}

#[command]
pub fn show_orb_context_menu(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    let new_chat = MenuItem::with_id(&app, "orb_new_chat", "新建对话", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let open_main = MenuItem::with_id(&app, "orb_open_main", "打开主窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(&app)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(&app, "orb_quit", "退出", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        &app,
        &[&new_chat, &open_main, &separator, &quit],
    )
    .map_err(|e| e.to_string())?;

    let webview_window = app.get_webview_window(WINDOW_ORB).ok_or("orb window not found")?;

    let window = webview_window.as_ref().window();
    menu.popup_at(window, tauri::LogicalPosition::new(x, y))
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}