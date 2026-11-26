// 参考: I78-serial-debug-spec.md §0.10 / §4.1 — 事件出口在此构造并注入领域层，
// 领域 crate 不依赖 tauri，可单测、可移植

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use assistant_config::constant::WINDOW_MAIN;
use assistant_serial::{EventSink, SerialEvent, SerialRuntime};

use super::{SERIAL_DEBUG_RX, SERIAL_DEBUG_STATUS};

/// `Rx` 定向主窗口（不广播，避免 orb 小窗重复序列化）；`Status` 广播
pub fn build_sink(app: &AppHandle) -> EventSink {
    let app = app.clone();
    Arc::new(move |event: SerialEvent| match event {
        SerialEvent::Rx(batch) => {
            let Some(window) = app.get_webview_window(WINDOW_MAIN) else {
                return;
            };
            if let Err(error) = window.emit(SERIAL_DEBUG_RX, batch) {
                log::error!("serial_debug:rx 发送失败: {}", error);
            }
        }
        SerialEvent::Status(snapshot) => {
            crate::protocol::events::emit_broadcast(&app, SERIAL_DEBUG_STATUS, snapshot);
        }
    })
}

pub fn install_sink(app: &AppHandle, rt: &SerialRuntime) {
    rt.set_sink(build_sink(app));
}
