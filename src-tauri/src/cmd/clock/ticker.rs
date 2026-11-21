// 参考: stretchly/app/utils/scheduler.js — 以墙钟重算剩余时间，而不是递减计数

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use assistant_clock::{now_ms, ClockRuntime, ClockSnapshot};
use assistant_protocol::ClockSnapshotDto;

use crate::protocol::events::emit_broadcast;

pub(crate) const CLOCK_FINISHED: &str = "clock:finished";

pub(crate) fn spawn_ticker(app: &AppHandle, rt: Arc<ClockRuntime>, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        if rt.generation() != generation {
            return;
        }
        let now = now_ms();
        let Some(remaining) = rt.remaining_ms(now) else {
            // 普通计时没有剩余时间，也没有到点语义。
            return;
        };
        if remaining <= 0 {
            if let Some(snapshot) = rt.mark_finished(now) {
                on_finished(&app, snapshot);
            }
            return;
        }
        let sleep_ms = u64::try_from(remaining.min(1000)).unwrap_or(1000);
        std::thread::sleep(Duration::from_millis(sleep_ms));
    });
}

fn on_finished(app: &AppHandle, snapshot: ClockSnapshot) {
    let dto = ClockSnapshotDto::from(snapshot);

    let needs_notification = app
        .get_webview_window(assistant_config::constant::WINDOW_MAIN)
        .map(|window| {
            !window.is_visible().unwrap_or(false) || !window.is_focused().unwrap_or(false)
        })
        .unwrap_or(true);

    if needs_notification && !send_notification(app) {
        log::warn!("Clock finished notification failed; showing main window as fallback");
        crate::window::show_or_create_main(app);
        if let Some(window) = app.get_webview_window(assistant_config::constant::WINDOW_MAIN) {
            let _ = window.set_focus();
        }
    }

    emit_broadcast(app, CLOCK_FINISHED, dto);
}

fn send_notification(app: &AppHandle) -> bool {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(assistant_i18n::t("clock.notification_title"))
        .body(assistant_i18n::t("clock.notification_body"))
        .show()
        .is_ok()
}
