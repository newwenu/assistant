// 参考: ChatGPT/src-tauri/src/core/conf.rs — tray 配置
// 参考: WindowPet/tray.rs — 菜单项和事件处理

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

use crate::protocol::events::{emit_broadcast, event_name};

use crate::window;
use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};

pub fn create_tray(app: &AppHandle) -> TrayIcon {
    let new_chat = MenuItem::with_id(
        app,
        "new_chat",
        assistant_i18n::t("tray.new_chat"),
        true,
        None::<&str>,
    )
    .expect("failed to create new_chat menu item");
    let show_main = MenuItem::with_id(
        app,
        "show_main",
        assistant_i18n::t("tray.show_main"),
        true,
        None::<&str>,
    )
    .expect("failed to create show_main menu item");
    let show_orb = MenuItem::with_id(
        app,
        "show_orb",
        assistant_i18n::t("tray.show_orb"),
        true,
        None::<&str>,
    )
    .expect("failed to create show_orb menu item");
    let screenshot = MenuItem::with_id(
        app,
        "screenshot",
        assistant_i18n::t("tray.screenshot"),
        true,
        None::<&str>,
    )
    .expect("failed to create screenshot menu item");
    let separator1 = PredefinedMenuItem::separator(app).expect("failed to create separator");
    let settings_general = MenuItem::with_id(
        app,
        "settings_general",
        assistant_i18n::t("tray.settings_general"),
        true,
        None::<&str>,
    )
    .expect("failed to create settings_general menu item");
    let settings_appearance = MenuItem::with_id(
        app,
        "settings_appearance",
        assistant_i18n::t("tray.settings_appearance"),
        true,
        None::<&str>,
    )
    .expect("failed to create settings_appearance menu item");
    let settings_shortcuts = MenuItem::with_id(
        app,
        "settings_shortcuts",
        assistant_i18n::t("tray.settings_shortcuts"),
        true,
        None::<&str>,
    )
    .expect("failed to create settings_shortcuts menu item");
    let separator2 = PredefinedMenuItem::separator(app).expect("failed to create separator");
    let quit = MenuItem::with_id(
        app,
        "quit",
        assistant_i18n::t("tray.quit"),
        true,
        None::<&str>,
    )
    .expect("failed to create quit menu item");

    let menu = Menu::with_items(
        app,
        &[
            &new_chat,
            &show_main,
            &show_orb,
            &screenshot,
            &separator1,
            &settings_general,
            &settings_appearance,
            &settings_shortcuts,
            &separator2,
            &quit,
        ],
    )
    .expect("failed to create tray menu");

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(assistant_i18n::t("app.name"))
        .on_menu_event(move |app: &AppHandle, event| match event.id.as_ref() {
            "new_chat" => {
                emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
            }
            "show_main" => {
                if let Err(e) = window::show_or_create(app, WINDOW_MAIN) {
                    log::warn!("tray show_main: show window {} failed: {e}", WINDOW_MAIN);
                }
            }
            "show_orb" => {
                if let Err(e) = window::show_or_create(app, WINDOW_ORB) {
                    log::warn!("tray show_orb: show window {} failed: {e}", WINDOW_ORB);
                }
            }
            "screenshot" => {
                // 与 Orb 右键菜单同一时序问题：点中菜单项后系统会播放关闭淡出，
                // 必须在点击这一刻隐藏残余菜单并锚定淡出起点，否则抓屏会冻进动画中间帧。
                crate::screenshot::orb_overlay::dismiss_menus_now();
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(code) = crate::screenshot::start(&handle).await {
                        crate::screenshot::emit_error(&handle, &code);
                    }
                });
            }
            "settings_general" => {
                if let Err(e) = window::show_or_create(app, WINDOW_MAIN) {
                    log::warn!(
                        "tray settings_general: show window {} failed: {e}",
                        WINDOW_MAIN
                    );
                }
                emit_broadcast(app, event_name::NAVIGATE, "/settings/general");
            }
            "settings_appearance" => {
                if let Err(e) = window::show_or_create(app, WINDOW_MAIN) {
                    log::warn!(
                        "tray settings_appearance: show window {} failed: {e}",
                        WINDOW_MAIN
                    );
                }
                emit_broadcast(app, event_name::NAVIGATE, "/settings/appearance");
            }
            "settings_shortcuts" => {
                if let Err(e) = window::show_or_create(app, WINDOW_MAIN) {
                    log::warn!(
                        "tray settings_shortcuts: show window {} failed: {e}",
                        WINDOW_MAIN
                    );
                }
                emit_broadcast(app, event_name::NAVIGATE, "/settings/shortcuts");
            }
            "quit" => {
                crate::cmd::perform_exit(app);
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
                if let Err(e) = window::show_or_create(tray.app_handle(), WINDOW_MAIN) {
                    log::warn!("tray icon click: show window {} failed: {e}", WINDOW_MAIN);
                }
            }
        })
        .build(app)
        .expect("failed to build tray icon")
}

pub fn rebuild_tray(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let new_chat = MenuItem::with_id(
            app,
            "new_chat",
            assistant_i18n::t("tray.new_chat"),
            true,
            None::<&str>,
        )
        .expect("failed to create new_chat menu item");
        let show_main = MenuItem::with_id(
            app,
            "show_main",
            assistant_i18n::t("tray.show_main"),
            true,
            None::<&str>,
        )
        .expect("failed to create show_main menu item");
        let show_orb = MenuItem::with_id(
            app,
            "show_orb",
            assistant_i18n::t("tray.show_orb"),
            true,
            None::<&str>,
        )
        .expect("failed to create show_orb menu item");
        let screenshot = MenuItem::with_id(
            app,
            "screenshot",
            assistant_i18n::t("tray.screenshot"),
            true,
            None::<&str>,
        )
        .expect("failed to create screenshot menu item");
        let separator1 = PredefinedMenuItem::separator(app).expect("failed to create separator");
        let settings_general = MenuItem::with_id(
            app,
            "settings_general",
            assistant_i18n::t("tray.settings_general"),
            true,
            None::<&str>,
        )
        .expect("failed to create settings_general menu item");
        let settings_appearance = MenuItem::with_id(
            app,
            "settings_appearance",
            assistant_i18n::t("tray.settings_appearance"),
            true,
            None::<&str>,
        )
        .expect("failed to create settings_appearance menu item");
        let settings_shortcuts = MenuItem::with_id(
            app,
            "settings_shortcuts",
            assistant_i18n::t("tray.settings_shortcuts"),
            true,
            None::<&str>,
        )
        .expect("failed to create settings_shortcuts menu item");
        let separator2 = PredefinedMenuItem::separator(app).expect("failed to create separator");
        let quit = MenuItem::with_id(
            app,
            "quit",
            assistant_i18n::t("tray.quit"),
            true,
            None::<&str>,
        )
        .expect("failed to create quit menu item");

        let menu = Menu::with_items(
            app,
            &[
                &new_chat,
                &show_main,
                &show_orb,
                &screenshot,
                &separator1,
                &settings_general,
                &settings_appearance,
                &settings_shortcuts,
                &separator2,
                &quit,
            ],
        )
        .expect("failed to create tray menu");

        tray.set_menu(Some(menu)).expect("failed to set tray menu");
    }
}
