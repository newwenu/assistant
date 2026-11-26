// 参考: I78-serial-debug-spec.md §6.1 / §4.7 — 一对互斥块，与既有插件同形

use tauri::plugin::TauriPlugin;

#[cfg(feature = "serial_debug")]
mod mounted {
    use super::TauriPlugin;

    use std::sync::Arc;

    use tauri::Manager;

    use assistant_serial::SerialRuntime;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("serial-debug")
            .invoke_handler(tauri::generate_handler![
                crate::cmd::serial_debug::serial_debug_list_ports,
                crate::cmd::serial_debug::serial_debug_open,
                crate::cmd::serial_debug::serial_debug_close,
                crate::cmd::serial_debug::serial_debug_status,
                crate::cmd::serial_debug::serial_debug_write,
                crate::cmd::serial_debug::serial_debug_set_signals,
            ])
            .build()
    }

    pub(crate) fn on_setup(ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        let rt = Arc::new(SerialRuntime::with_default_backend());
        crate::cmd::serial_debug::reader::install_sink(ctx.app, &rt);
        ctx.app.manage(rt);
        Ok(())
    }

    /// 停线程、关端口；未开时幂等返回。**不发事件**（窗口已销毁）
    pub(crate) fn on_exit(app: &tauri::AppHandle) {
        let Some(rt) = app.try_state::<Arc<SerialRuntime>>() else {
            return;
        };
        rt.close_quietly();
    }
}

#[cfg(not(feature = "serial_debug"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("serial-debug").build()
    }

    pub(crate) fn on_setup(_ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};
