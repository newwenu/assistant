// 参考: docs/store/I75-startup-layering-spec.md §10 — 启动报告命令

use std::sync::OnceLock;

use tauri::{command, AppHandle, Manager};

use crate::boot::{BootClock, StartupReport};
use crate::protocol::dto::{CapabilityStateDto, StartupReportDto};

/// Idle pass 一次性触发开关：首帧之后预热一次，之后不再重复。
static IDLE_STARTED: OnceLock<()> = OnceLock::new();

/// 前端首帧回传。
///
/// 时钟域由后端 `t0_epoch_ms` 定义：前端只上报「首帧绘制时刻的 epoch 毫秒」，
/// 时延在后端相减，避免两进程各自的 `Instant` 基准不同。
///
/// 两种轮次共用一条命令：
/// - `startup_id` 等于进程 id：本轮启动的首帧（T_visible），写入报告；
/// - `wake-<uuid>`：主窗口懒建轮次的首帧（T_wake），起点在 `show_or_create_main` 打点，
///   只记日志（T_wake 是独立指标，不覆盖 T_visible）。
///
/// 其余 id 说明回传来自过期轮次，记录告警并丢弃，不让陈旧数据污染本轮指标。
#[command]
pub fn report_first_frame(
    app: AppHandle,
    startup_id: String,
    frame_epoch_ms: i64,
) -> Result<(), String> {
    let clock = app.state::<BootClock>();
    let report = app.state::<StartupReport>();
    if clock.startup_id() == startup_id {
        report.record_first_frame(clock.inner(), frame_epoch_ms);
    } else if let Some(wake_start_ms) = report.take_wake_start(&startup_id) {
        let wake_ms = u64::try_from(frame_epoch_ms.saturating_sub(wake_start_ms)).unwrap_or(0);
        log::info!("startup wake_first_frame startup_id={startup_id} wake_ms={wake_ms}");
    } else {
        log::warn!(
            "first frame startup_id mismatch: expected {} got {}",
            clock.startup_id(),
            startup_id
        );
        return Ok(());
    }

    // Idle 泳道没有独立调度器：首帧落地即预热一次，与首次使用共用 `ensure`。
    if IDLE_STARTED.set(()).is_ok() {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::boot::run_idle(&handle).await;
        });
    }
    Ok(())
}

#[command]
pub fn get_startup_report(app: AppHandle) -> StartupReportDto {
    let clock = app.state::<BootClock>();
    app.state::<StartupReport>().snapshot(clock.inner())
}

/// 能力状态水合（I75 §4.4 C4 / C5）：前端首帧后拉一次全表快照，
/// 之后只靠 `startup:capability-changed` 增量刷新，不轮询。
#[command]
pub fn get_capability_states(app: AppHandle) -> Vec<CapabilityStateDto> {
    app.state::<crate::boot::Registry>().snapshot()
}

/// 重试入口（C5）：对 `Failed` 且 `retryable` 的表项重新走 `ensure`。
///
/// 命令只负责触发；结果（`Ready` / 仍失败）经 `startup:capability-changed` 广播，
/// 前端不依赖本命令的返回值判定成功与否。
#[command]
pub async fn retry_capability(app: AppHandle, id: String) -> Result<(), String> {
    let res = crate::boot::find(&id).ok_or_else(|| format!("unknown capability: {id}"))?;
    crate::boot::ensure(&app, res.id)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
