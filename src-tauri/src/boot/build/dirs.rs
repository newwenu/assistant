// 参考: docs/store/I75-startup-layering-spec.md §9 #1 — `dirs` 由 `run_boot` 注入，不参与装配

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};

/// 目录解析的唯一发生在 `main.rs`；槽由 `run_boot` 在表序执行前注入。
/// 本函数只在注入缺失时被调用，因此直接返回 `NotReady`，绝不二次解析。
pub fn build(_ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    Err(CapErr::not_ready("dirs"))
}
