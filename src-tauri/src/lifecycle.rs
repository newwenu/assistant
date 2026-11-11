// 参考: BongoCat/crates/bongocat-app/src/lib.rs — 模块化组织

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

pub fn request_exit() {
    SHOULD_EXIT.store(true, Ordering::Relaxed);
}

pub fn should_exit() -> bool {
    SHOULD_EXIT.load(Ordering::Relaxed)
}

pub fn on_run_event(app_handle: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Exit = event {
        for mount in crate::feature_mounts::mounts() {
            (mount.on_exit)(app_handle);
        }
    }
    if let tauri::RunEvent::ExitRequested { api, .. } = event {
        if !should_exit() {
            api.prevent_exit();
        }
    }
}
