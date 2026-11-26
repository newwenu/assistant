// 参考: docs/store/I75-startup-layering-spec.md §3.2 / §4.3 — 表序执行与 severity 分派

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::app_dirs::AppDirs;
use crate::boot::ctx::{BootClock, Outcome, StartupCtx, StartupReport};
use crate::boot::registry::Registry;
use crate::boot::resources::{severity, Lane, Res, Severity, RESOURCES};

/// `Boot` 泳道：按表序执行，每项一条阶段行，失败语义由集合成员派生。
pub fn run_boot(app: &AppHandle, dirs: AppDirs) -> Result<(), Box<dyn std::error::Error>> {
    let clock = app.state::<BootClock>();
    let report = app.state::<StartupReport>();
    let registry = app.state::<Registry>();
    registry.inject("dirs", Arc::new(dirs));

    let ctx = StartupCtx::new(app, &clock, &report, &registry);
    for res in RESOURCES.iter().filter(|res| res.lane == Lane::Boot) {
        run_one(&ctx, res)?;
    }
    log::info!("Assistant started");
    Ok(())
}

/// `Idle` 泳道：Orb 首帧后一次性预热，与首次使用共用 `ensure` 的同一份装配。
pub fn run_idle(ctx: &StartupCtx<'_>) {
    for res in RESOURCES.iter().filter(|res| res.lane == Lane::Idle) {
        if ctx.report.has_phase(res.id) {
            continue;
        }
        run_one(ctx, res).ok();
        std::thread::yield_now();
    }
}

fn run_one(ctx: &StartupCtx<'_>, res: &'static Res) -> Result<(), Box<dyn std::error::Error>> {
    match severity(res.id) {
        Severity::Fatal => {
            ctx.report
                .stage(ctx.clock, res.id, res.lane, Outcome::Fatal, || {
                    ctx.build(res).map(|_| ())
                })?;
        }
        Severity::Degraded => {
            // 降档已由 `Registry::build` 统一记一次 warn：boot / idle / `ensure` /
            // 重试四条路径共用同一处，这里只负责阶段行，不重复记同一失败。
            ctx.report.stage_ignored(ctx.clock, res.id, res.lane, || {
                ctx.build(res).map(|_| ()).map_err(|e| e.to_string())
            });
        }
        Severity::Ignore => {
            ctx.report
                .stage_best_effort(ctx.clock, res.id, res.lane, || {
                    ctx.build(res).map(|_| ()).map_err(|e| e.to_string())
                });
        }
    }
    Ok(())
}
