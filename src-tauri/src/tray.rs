// 参考: ChatGPT/src-tauri/src/core/conf.rs — tray 配置
// 参考: WindowPet/tray.rs — 菜单项和事件处理

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter,
};

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
        .tooltip(&assistant_i18n::t("app.name"))
        .on_menu_event(move |app: &AppHandle, event| match event.id.as_ref() {
            "new_chat" => {
                let _ = app.emit("orb-new-conversation", ());
            }
            "show_main" => {
                let _ = window::show_or_create(app, WINDOW_MAIN);
            }
            "show_orb" => {
                let _ = window::show_or_create(app, WINDOW_ORB);
            }
            "settings_general" => {
                let _ = window::show_or_create(app, WINDOW_MAIN);
                let _ = app.emit("navigate", "/settings/general");
            }
            "settings_appearance" => {
                let _ = window::show_or_create(app, WINDOW_MAIN);
                let _ = app.emit("navigate", "/settings/appearance");
            }
            "settings_shortcuts" => {
                let _ = window::show_or_create(app, WINDOW_MAIN);
                let _ = app.emit("navigate", "/settings/shortcuts");
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
                let _ = window::show_or_create(tray.app_handle(), WINDOW_MAIN);
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
