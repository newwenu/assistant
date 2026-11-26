// 参考: docs/store/I75-startup-layering-spec.md §9 #10 / #13 — 窗口类装配

use std::sync::Arc;

use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};

/// 主入口：建窗（规格 + 几何同源）→ 定位 → 属性 → `show`，合并为一项。
pub fn build_orb_shell(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let dirs = ctx.dirs()?;
    crate::window::create::create_window(ctx.app, WINDOW_ORB, &dirs)
        .map_err(|e| CapErr::build("orb_shell", e))?;
    crate::window::create::show_window(ctx.app, WINDOW_ORB)
        .map_err(|e| CapErr::build("orb_shell", e))?;

    // `E14` 漂移守卫（I67 §13.3）：子目录名 `EBWebView` 由 WebView2 固定创建，仓库内没有
    // 可比对的常量，因此建窗后校验存在性——Tauri / WebView2 若改落点或改名，启动日志
    // 直接暴露，不会静默把字节漏在 `unregistered`。
    #[cfg(target_os = "windows")]
    if !dirs.webview_profile().exists() {
        log::warn!(
            "WebView2 profile dir missing after window creation; E14 will be reported missing: {}",
            dirs.webview_profile().display()
        );
    }

    Ok(Arc::new(()))
}

/// 主窗口懒建：`Idle` 档建成后保持隐藏，几何随建窗路径一次套用；显示由入口触发。
pub fn build_window_main(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let dirs = ctx.dirs()?;
    crate::window::create::create_window(ctx.app, WINDOW_MAIN, &dirs)
        .map_err(|e| CapErr::build("window_main", e))?;
    Ok(Arc::new(()))
}
