// 参考: feature-mount-traversal-spec.md（已归档） §3 — 可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "clock")]
mod mounted {
    use super::TauriPlugin;

    use std::sync::Arc;

    use tauri::Manager;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("clock")
            .invoke_handler(tauri::generate_handler![
                crate::cmd::clock::clock_set_mode,
                crate::cmd::clock::clock_start,
                crate::cmd::clock::clock_pause,
                crate::cmd::clock::clock_resume,
                crate::cmd::clock::clock_reset,
                crate::cmd::clock::clock_snapshot,
            ])
            .build()
    }

    pub(crate) fn on_setup(app: &tauri::AppHandle) -> Result<(), String> {
        app.manage(Arc::new(assistant_clock::ClockRuntime::new()));
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

#[cfg(not(feature = "clock"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("clock").build()
    }

    pub(crate) fn on_setup(_app: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};
