// 参考: docs/store/I75-startup-layering-spec.md §9 #9 / #11 / #12 — 托盘、巡检、快捷键

use std::sync::Arc;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};

pub fn build_tray(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    crate::tray::create_tray(ctx.app);
    Ok(Arc::new(()))
}

/// 本项只 spawn；线程体（每轮现读并回写 `OrbConf`）留在 `window/placement.rs`。
pub fn build_screen_inspection(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    crate::window::placement::spawn_screen_inspection(ctx.app.clone());
    Ok(Arc::new(()))
}

/// 快捷键实体在 `shortcuts.rs`（运行期改配置也要调用），本项只调用并传配置快照。
pub fn build_shortcuts(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let conf = ctx.boot()?;
    crate::shortcuts::register_global_shortcuts(ctx.app, &conf.app);
    Ok(Arc::new(()))
}
