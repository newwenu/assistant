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

use assistant_config::{
    app_conf::AppConf,
    constant::{
        MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MINI_HEIGHT,
        MAIN_WINDOW_MINI_MIN_HEIGHT, MAIN_WINDOW_MINI_MIN_WIDTH, MAIN_WINDOW_MINI_WIDTH,
        MAIN_WINDOW_MIN_HEIGHT, MAIN_WINDOW_MIN_WIDTH, ORB_CHROME_INSET, ORB_COLLAPSED_SIZE,
        ORB_COLLAPSED_WINDOW_SIZE, ORB_EXPANDED_HEIGHT, ORB_EXPANDED_WIDTH,
        ORB_EXPANDED_WINDOW_HEIGHT, ORB_EXPANDED_WINDOW_WIDTH, WINDOW_MAIN, WINDOW_ORB,
        WINDOW_SETTINGS,
    },
};
use assistant_orb::placement::{self, WindowBounds};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{WindowModeChangedPayloadDto, WindowModeDto};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainWindowMode {
    Normal,
    Mini,
}

impl MainWindowMode {
    fn as_dto(self) -> WindowModeDto {
        match self {
            MainWindowMode::Normal => WindowModeDto::Normal,
            MainWindowMode::Mini => WindowModeDto::Mini,
        }
    }
}

impl From<WindowModeDto> for MainWindowMode {
    fn from(value: WindowModeDto) -> Self {
        match value {
            WindowModeDto::Normal => MainWindowMode::Normal,
            WindowModeDto::Mini => MainWindowMode::Mini,
        }
    }
}

static MAIN_WINDOW_MODE: LazyLock<Mutex<MainWindowMode>> =
    LazyLock::new(|| Mutex::new(MainWindowMode::Normal));

pub fn current_mode() -> MainWindowMode {
    *MAIN_WINDOW_MODE.lock().unwrap_or_else(|e| e.into_inner())
}

fn set_mode(mode: MainWindowMode) {
    *MAIN_WINDOW_MODE.lock().unwrap_or_else(|e| e.into_inner()) = mode;
}

/// mini 恒置顶；normal 遵循用户设置。
pub fn effective_on_top(stay_on_top: bool) -> bool {
    current_mode() == MainWindowMode::Mini || stay_on_top
}

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

fn logical_scale(window: &tauri::WebviewWindow) -> Result<f64, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if scale <= 0.0 {
        return Err("scale_factor returned non-positive value".to_string());
    }
    Ok(scale)
}

fn saved_normal_size(conf: &AppConf) -> (f64, f64) {
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

fn snap_position(
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

/// mini 形态是运行时状态，不落盘；启动时只恢复 normal 几何。
pub fn restore_main_window(app: &AppHandle) -> Result<(), String> {
    let window = get_or_error(app, WINDOW_MAIN)?;
    let conf = AppConf::load().map_err(|e| e.to_string())?;

    let (width, height) = saved_normal_size(&conf);
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;

    if let (Some(x), Some(y)) = (conf.main_window_x, conf.main_window_y) {
        snap_position(app, &window, x, y)?;
    }

    set_mode(MainWindowMode::Normal);
    Ok(())
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

#[command]
pub fn set_main_window_mode(app: AppHandle, mode: WindowModeDto) -> Result<(), String> {
    let target = MainWindowMode::from(mode);
    if current_mode() == target {
        return Ok(());
    }

    persist_main_geometry(&app)?;
    let window = get_or_error(&app, WINDOW_MAIN)?;
    let conf = AppConf::load().map_err(|e| e.to_string())?;

    match target {
        MainWindowMode::Mini => apply_mini(&window, &conf)?,
        MainWindowMode::Normal => apply_normal(&window, &conf)?,
    }
    set_mode(target);
    emit_broadcast(
        &app,
        event_name::WINDOW_MODE_CHANGED,
        WindowModeChangedPayloadDto {
            mode: target.as_dto(),
        },
    );
    Ok(())
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
                let _ = persist_main_geometry(window.app_handle());
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
