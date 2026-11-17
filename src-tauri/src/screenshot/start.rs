// 开始截屏：选屏 → 隐藏 Orb → 抓屏编码 → 建会话 → 下发 → 铺满显影。

use std::sync::Arc;
use std::time::{Duration, Instant};

use log::info;
use tauri::{AppHandle, Manager};

use assistant_config::constant::SCREENSHOT_OVERLAY_SETTLE_MS;

use crate::protocol::ScreenshotSessionDto;

use super::capture;
use super::cleanup::{prune_stale, reset_orphans};
use super::orb_overlay;
use super::state::{discard_session, set_phase, PendingSession, Phase, ScreenshotState, Target};
use super::{CAPTURE_FAILED, NO_MONITOR, PERMISSION_DENIED};

/// 分段耗时：t0 取「菜单项被按下」那一刻（无菜单入口时退回本函数进入时刻），
/// 每段打一行 `stage=` 日志，用实测数据定位瓶颈，不靠固定常量猜。
struct StageTimer(Instant);

impl StageTimer {
    fn new() -> Self {
        Self(orb_overlay::menu_anchor().unwrap_or_else(Instant::now))
    }

    fn mark(&self, stage: &str) {
        info!(
            "screenshot: stage={stage} +{}ms",
            self.0.elapsed().as_millis()
        );
    }
}

/// 开始截屏：选屏 → 隐藏 Orb → 抓屏编码 → 建会话 → 下发 → 铺满显影。
pub async fn start(app: &AppHandle) -> Result<(), String> {
    let timer = StageTimer::new();
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    prune_stale(app, &state);
    reset_orphans(app, &state);

    {
        let inner = state.lock_inner();
        if inner.phase != Phase::Idle {
            info!("screenshot: start ignored, phase = {:?}", inner.phase);
            return Ok(());
        }
    }

    if !capture::permission_granted(app) {
        return Err(PERMISSION_DENIED.to_string());
    }

    // 菜单/托盘入口不经过前端工具栏，前端此刻仍是「小球形态」；而下面会把同一个窗口
    // 撑成全屏，小球 UI 的绝对定位会落到新窗口的原点（左上角）。先通知前端进入覆盖层
    // 模式隐藏球体，再往下走——后续的隐藏序列、菜单淡出排空与抓帧编码有足够时间让前端
    // 应用这个状态，等到 `present` 显影时窗口里已经是覆盖层而不是小球。
    crate::protocol::emit_orb_only(app, crate::protocol::event_name::SCREENSHOT_PREPARING, ());

    let backend = state.backend.clone();
    let monitors = tokio::task::spawn_blocking(move || backend.monitors())
        .await
        .map_err(|_| NO_MONITOR.to_string())?
        .map_err(|_| NO_MONITOR.to_string())?;
    timer.mark("monitors");

    let point = capture::cursor_point(app);
    let monitor =
        capture::select_monitor(&monitors, point).ok_or_else(|| NO_MONITOR.to_string())?;
    let scale_factor = capture::scale_factor_for(app, monitor);

    let snapshot = orb_overlay::begin(app)?;
    timer.mark("orb-hidden");

    {
        let mut inner = state.lock_inner();
        inner.phase = Phase::Capturing;
    }

    let captured = {
        let backend = state.backend.clone();
        tokio::task::spawn_blocking(move || {
            let image = capture::capture_monitor(backend.as_ref(), monitor)?;
            let preview = capture::encode_preview(&image)?;
            Ok::<_, String>((image, preview))
        })
        .await
        .map_err(|_| CAPTURE_FAILED.to_string())?
    };
    timer.mark("captured");

    let (image, preview) = match captured {
        Ok(pair) => pair,
        Err(error) => {
            let _ = orb_overlay::exit(app, snapshot);
            set_phase(&state, Phase::Idle);
            return Err(error);
        }
    };

    let target = Target {
        monitor_x: monitor.x,
        monitor_y: monitor.y,
        monitor_width: monitor.width,
        monitor_height: monitor.height,
        scale_factor,
    };
    info!(
        "screenshot: target {}x{} at ({}, {}), scale {:.2}, preview {}x{}",
        target.monitor_width,
        target.monitor_height,
        target.monitor_x,
        target.monitor_y,
        target.scale_factor,
        preview.width,
        preview.height
    );

    let token = uuid::Uuid::new_v4().simple().to_string();
    let session_dto = ScreenshotSessionDto {
        token: token.clone(),
        preview: preview.data_url.clone(),
        preview_width: preview.width,
        preview_height: preview.height,
    };

    {
        let mut inner = state.lock_inner();
        inner.session = Some(PendingSession {
            token,
            preview: preview.data_url,
            preview_width: preview.width,
            preview_height: preview.height,
            image: Arc::new(image),
            target,
            created_at: Instant::now(),
            orb: snapshot,
        });
        inner.phase = Phase::Selecting;
    }

    // 会话数据必须在 Orb 仍隐藏时下发，覆盖层一显影就有底图。
    crate::protocol::emit_orb_only(
        app,
        crate::protocol::event_name::SCREENSHOT_SESSION,
        session_dto,
    );

    if let Err(error) = orb_overlay::present(app, monitor) {
        let discarded = discard_session(&state);
        if let Some(session) = discarded {
            let _ = orb_overlay::exit(app, session.orb);
        }
        return Err(error);
    }
    timer.mark("presented");

    tokio::time::sleep(Duration::from_millis(SCREENSHOT_OVERLAY_SETTLE_MS)).await;
    Ok(())
}
