// 参考: ChatGPT/src-tauri/src/main.rs — 菜单事件集中分发

use assistant_config::constant::WINDOW_MAIN;
use tauri::AppHandle;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::{cmd, window};

pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "orb_new_chat" => {
            emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
        }
        "orb_open_main" => {
            let _ = window::show_or_create(app, WINDOW_MAIN);
        }
        "orb_screenshot" => {
            // 点击这一刻就锚定菜单淡出起点：`popup_at` 之后才记时刻会晚 10~30ms，
            // 且抓屏前枚举不到菜单窗口（已被销毁）时连等待都会被跳过，动画帧照样被冻进底图。
            crate::screenshot::orb_overlay::dismiss_menus_now();
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(code) = crate::screenshot::start(&handle).await {
                    crate::screenshot::emit_error(&handle, &code);
                }
            });
        }
        "orb_quit" => {
            cmd::perform_exit(app);
        }
        _ => {}
    }
}
