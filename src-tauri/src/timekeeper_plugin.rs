// 参考: feature-mount-traversal-spec.md（已归档） §3 — 可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "timekeeper")]
mod mounted {
    use super::TauriPlugin;

    use std::sync::Arc;

    use tauri::Manager;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("timekeeper")
            .invoke_handler(tauri::generate_handler![
                crate::cmd::timekeeper::timekeeper_set_mode,
                crate::cmd::timekeeper::timekeeper_start,
                crate::cmd::timekeeper::timekeeper_pause,
                crate::cmd::timekeeper::timekeeper_resume,
                crate::cmd::timekeeper::timekeeper_reset,
                crate::cmd::timekeeper::timekeeper_snapshot,
            ])
            .build()
    }

    pub(crate) fn on_setup(ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        ctx.app
            .manage(Arc::new(assistant_clock::ClockRuntime::new()));
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

#[cfg(not(feature = "timekeeper"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("timekeeper").build()
    }

    pub(crate) fn on_setup(_ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};
