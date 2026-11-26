// 参考: docs/store/I75-startup-layering-spec.md §5 — 主窗口形态与事件

use std::sync::{LazyLock, Mutex};

use tauri::{command, Manager};

use assistant_config::constant::WINDOW_MAIN;

use crate::protocol::WindowModeDto;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainWindowMode {
    Normal,
    Mini,
}

impl MainWindowMode {
    pub(crate) fn as_dto(self) -> WindowModeDto {
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

pub(crate) fn set_mode(mode: MainWindowMode) {
    *MAIN_WINDOW_MODE.lock().unwrap_or_else(|e| e.into_inner()) = mode;
}

/// mini 恒置顶；normal 遵循用户设置。
pub fn effective_on_top(stay_on_top: bool) -> bool {
    current_mode() == MainWindowMode::Mini || stay_on_top
}

#[command]
pub fn set_main_window_mode(app: tauri::AppHandle, mode: WindowModeDto) -> Result<(), String> {
    super::geometry::apply_mode(&app, MainWindowMode::from(mode))
}

#[command]
pub async fn open_main_window(app: tauri::AppHandle) -> Result<(), String> {
    super::create::show_main_window(&app).await
}

pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if window.label() == assistant_config::constant::WINDOW_ORB
                && crate::screenshot::is_active()
            {
                api.prevent_close();
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::screenshot::cancel_active(&app).await {
                        log::warn!("cancel_active on orb close failed: {e}");
                    }
                });
                return;
            }
            if window.label() == WINDOW_MAIN {
                if let Err(e) = super::geometry::persist_main_geometry(window.app_handle()) {
                    log::warn!("persist_main_geometry failed: {e}");
                }
                let close_to_tray = assistant_config::app_conf::AppConf::load()
                    .map(|c| c.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    if let Err(e) = window.hide() {
                        log::warn!("hide main window failed: {e}");
                    }
                }
            }
        }
        tauri::WindowEvent::Focused(false)
            if window.label() == assistant_config::constant::WINDOW_ORB =>
        {
            if crate::screenshot::is_active() {
                return;
            }
            use tauri::Emitter;
            if let Err(e) = window.emit(crate::protocol::events::event_name::ORB_BLUR, ()) {
                log::warn!(
                    "emit {} failed: {e}",
                    crate::protocol::events::event_name::ORB_BLUR
                );
            }
        }
        _ => {}
    }
}
