// 参考: docs/store/I75-startup-layering-spec.md §9 #3 / #4 / #7 — 进程内状态类装配

use std::sync::Arc;

use tauri::Manager;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};

/// 语言只取自 `BootConf`，不再二次 `load`。
pub fn build_i18n(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let conf = ctx.boot()?;
    assistant_i18n::init_global(&conf.app.language);
    Ok(Arc::new(()))
}

/// 特性 `on_setup` 循环：单个特性失败只记日志，全部失败才向本阶段报错。
pub fn build_feature_on_setup(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let mut failures = Vec::new();
    for mount in crate::feature_mounts::mounts() {
        if let Err(e) = (mount.on_setup)(ctx) {
            log::error!("[{}] {}", mount.id, e);
            failures.push(mount.id);
        }
    }
    if failures.is_empty() {
        Ok(Arc::new(()))
    } else {
        Err(CapErr::build(
            "feature_on_setup",
            format!("on_setup 失败特性: {}", failures.join(", ")),
        ))
    }
}

/// 五个进程内状态槽：不依赖配置，只按类型 `manage`。
pub fn build_runtime_state(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    ctx.app
        .manage(Arc::new(assistant_agent::kernel::TurnTasks::new()));
    ctx.app
        .manage(Arc::new(assistant_agent::kernel::SessionStates::new()));
    ctx.app.manage(crate::screenshot::ScreenshotState::new());
    ctx.app
        .manage(crate::cmd::storage::inventory::InventoryCache::new());
    ctx.app.manage(Arc::new(
        crate::cmd::storage::reclaim::ReclaimPlanStore::new(),
    ));
    Ok(Arc::new(()))
}
