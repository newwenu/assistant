// 参考: ChatGPT/src-tauri/src/core/window.rs — 动态创建窗口
// 参考: BongoCat overlay placement — 窗口尺寸管理

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use log::info;
use tauri::{
    command,
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

use assistant_config::constant::{
    MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MIN_HEIGHT,
    MAIN_WINDOW_MIN_WIDTH, ORB_CHROME_INSET, ORB_COLLAPSED_SIZE, ORB_COLLAPSED_WINDOW_SIZE,
    ORB_EXPANDED_HEIGHT, ORB_EXPANDED_WIDTH, ORB_EXPANDED_WINDOW_HEIGHT, ORB_EXPANDED_WINDOW_WIDTH,
    WINDOW_MAIN, WINDOW_ORB, WINDOW_SETTINGS,
};

use crate::protocol::events::event_name;

pub struct WindowSpec {
    pub label: &'static str,
    pub url: &'static str,
    pub title: &'static str,
    pub width: f64,
    pub height: f64,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub center: bool,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub always_on_top: bool,
    pub skip_taskbar: bool,
}

static WINDOW_SPECS: LazyLock<HashMap<&'static str, WindowSpec>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(
        WINDOW_MAIN,
        WindowSpec {
            label: WINDOW_MAIN,
            url: "/",
            title: "Assistant",
            width: MAIN_WINDOW_DEFAULT_WIDTH,
            height: MAIN_WINDOW_DEFAULT_HEIGHT,
            min_width: Some(MAIN_WINDOW_MIN_WIDTH),
            min_height: Some(MAIN_WINDOW_MIN_HEIGHT),
            center: true,
            resizable: true,
            decorations: false,
            transparent: false,
            always_on_top: false,
            skip_taskbar: false,
        },
    );
    m.insert(
        WINDOW_SETTINGS,
        WindowSpec {
            label: WINDOW_SETTINGS,
            url: "/settings",
            title: "Settings",
            width: 600.0,
            height: 500.0,
            min_width: None,
            min_height: None,
            center: true,
            resizable: true,
            decorations: false,
            transparent: false,
            always_on_top: false,
            skip_taskbar: false,
        },
    );
    m
});

static WINDOW_LOCKS: LazyLock<Mutex<HashMap<String, ()>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn show_or_create(app: &AppHandle, label: &str) -> Result<(), String> {
    let _lock = WINDOW_LOCKS.lock().unwrap();

    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let spec = WINDOW_SPECS.get(label).ok_or_else(|| {
        let mut args = std::collections::HashMap::new();
        args.insert("label", label);
        assistant_i18n::t_with_args("window.unknown_label", &args)
    })?;

    let resolved_title = match label {
        WINDOW_MAIN => assistant_i18n::t("window.title_main"),
        WINDOW_SETTINGS => assistant_i18n::t("window.title_settings"),
        _ => spec.title.to_string(),
    };

    let mut builder = WebviewWindowBuilder::new(app, spec.label, WebviewUrl::App(spec.url.into()))
        .title(&resolved_title)
        .inner_size(spec.width, spec.height)
        .resizable(spec.resizable)
        .decorations(spec.decorations)
        .transparent(spec.transparent)
        .always_on_top(spec.always_on_top)
        .skip_taskbar(spec.skip_taskbar);

    if spec.center {
        builder = builder.center();
    }

    if let (Some(mw), Some(mh)) = (spec.min_width, spec.min_height) {
        builder = builder.min_inner_size(mw, mh);
    }

    #[cfg(not(debug_assertions))]
    let builder = builder.devtools(false);

    if let Err(e) = builder.build() {
        log::warn!("Failed to build window {}: {}", label, e);
        if let Some(window) = app.get_webview_window(label) {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    info!("Created window: {}", label);
    Ok(())
}

pub fn toggle(app: &AppHandle, label: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        show_or_create(app, label)
    }
}

pub fn get_or_error(app: &AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("{} window not found", label))
}

pub fn destroy_all(app: &AppHandle, labels: &[&str]) {
    for label in labels {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
    }
}

pub fn show_or_create_main(app: &AppHandle) {
    let _ = show_or_create(app, WINDOW_MAIN);
}

#[command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    show_or_create(&app, WINDOW_MAIN)
}

#[command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    show_or_create(&app, WINDOW_SETTINGS)
}

#[command]
pub fn expand_orb_panel(
    app: AppHandle,
    horizontal: String,
    vertical: String,
) -> Result<(), String> {
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
    let separator = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(
        &app,
        "orb_quit",
        assistant_i18n::t("orb.context_quit"),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(&app, &[&new_chat, &open_main, &separator, &quit])
        .map_err(|e| e.to_string())?;

    let webview_window = get_or_error(&app, WINDOW_ORB)?;

    let window = webview_window.as_ref().window();
    menu.popup_at(window, tauri::LogicalPosition::new(x, y))
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}

pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if window.label() == WINDOW_MAIN {
                let close_to_tray = assistant_config::app_conf::AppConf::load()
                    .map(|c| c.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        }
        tauri::WindowEvent::Focused(false) => {
            if window.label() == WINDOW_ORB {
                use tauri::Emitter;
                let _ = window.emit(event_name::ORB_BLUR, ());
            }
        }
        _ => {}
    }
}
