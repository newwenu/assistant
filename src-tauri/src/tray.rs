// 参考: ChatGPT/src-tauri/src/core/conf.rs — tray 配置
// 参考: WindowPet/tray.rs — 菜单项和事件处理

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};
use crate::{request_exit, window::show_or_create_main};

pub fn create_tray(app: &AppHandle) -> TrayIcon {
    let show_main = MenuItem::with_id(app, "show_main", "打开主窗口", true, None::<&str>)
        .expect("failed to create show_main menu item");
    let show_orb = MenuItem::with_id(app, "show_orb", "显示悬浮球", true, None::<&str>)
        .expect("failed to create show_orb menu item");
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .expect("failed to create quit menu item");
    let separator = PredefinedMenuItem::separator(app)
        .expect("failed to create separator");

    let menu = Menu::with_items(
        app,
        &[&show_main, &show_orb, &separator, &quit],
    )
    .expect("failed to create tray menu");

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Assistant")
        .on_menu_event(move |app: &AppHandle, event| match event.id.as_ref() {
            "show_main" => {
                show_or_create_main(app);
            }
            "show_orb" => {
                if let Some(window) = app.get_webview_window(WINDOW_ORB) {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                request_exit();
                if let Some(w) = app.get_webview_window(WINDOW_ORB) {
                    let _ = w.destroy();
                }
                if let Some(w) = app.get_webview_window(WINDOW_MAIN) {
                    let _ = w.destroy();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_or_create_main(tray.app_handle());
            }
        })
        .build(app)
        .expect("failed to build tray icon")
}