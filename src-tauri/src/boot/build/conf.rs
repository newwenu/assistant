// 参考: docs/store/I75-startup-layering-spec.md §9 #2 — 配置快照装配

use std::sync::Arc;

use crate::boot::conf::BootConf;
use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};

/// 失败不终止启动：回落默认值并逐项记 `warn`（`BootConf::load_all` 永不返回 `Err`）。
pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let _ = ctx;
    let (conf, failures) = BootConf::load_all();
    for failure in failures {
        log::warn!("boot config degraded: {failure}");
    }
    Ok(Arc::new(conf))
}
