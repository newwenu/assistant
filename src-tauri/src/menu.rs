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
        "orb_quit" => {
            cmd::perform_exit(app);
        }
        _ => {}
    }
}
