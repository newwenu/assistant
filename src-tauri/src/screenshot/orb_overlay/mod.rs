// Orb 悬浮球复用为全屏覆盖层：隐藏序列 + 快照恢复
// 参考: Kiri ADR 0107 — 禁用 DWM 过渡动画 + 可见性校验 + ShowWindow 兜底 + DwmFlush

mod menu_fade;
mod platform;
mod snapshot;

use std::sync::atomic::{AtomicBool, Ordering};

use log::{info, warn};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use assistant_config::{
    constant::{ORB_COLLAPSED_WINDOW_SIZE, WINDOW_ORB},
    orb_conf::OrbConf,
};

use super::capture::MonitorInfo;
use super::NO_SESSION;
use snapshot::{
    apply_geometry, hide_and_verify, reset_collapsed, restore_with, snapshot_of, WebviewOps,
    WindowOps,
};

/// 截屏会话标志：置位期间 Orb 巡检、失焦折叠、位置持久化一律跳过。
pub static SCREENSHOT_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    SCREENSHOT_ACTIVE.load(Ordering::Acquire)
}

pub(crate) fn set_active(active: bool) {
    SCREENSHOT_ACTIVE.store(active, Ordering::Release);
}

/// 覆盖层出现前的 Orb 现场：会话结束按此原样还原。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbWindowSnapshot {
    pub position: PhysicalPosition<i32>,
    pub size: PhysicalSize<u32>,
    pub visible: bool,
    pub always_on_top: bool,
    pub click_through: bool,
}

fn orb_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(WINDOW_ORB)
        .ok_or_else(|| NO_SESSION.to_string())
}

/// 进入截屏：先置标志再隐藏，保证隐藏过程触发的窗口事件被守卫拦下。
pub fn begin(app: &AppHandle) -> Result<OrbWindowSnapshot, String> {
    let window = orb_window(app)?;
    let snapshot = snapshot_of(&window)?;
    set_active(true);
    if let Err(error) = hide_and_verify(&window) {
        // 隐藏序列可能只做了一半（例如 `hide` 已落地但可见性查询失败）：
        // 按快照恢复可见性再清标志，避免 Orb 永久消失。
        let _ = restore_with(&mut WebviewOps(&window), snapshot, |enabled| {
            crate::cmd::orb::apply_orb_click_through(app, enabled)
        });
        set_active(false);
        return Err(error);
    }
    info!("screenshot: orb hidden");
    Ok(snapshot)
}

/// 铺满目标显示器并显影；调用方负责在显影前下发会话数据。
pub fn present(app: &AppHandle, monitor: MonitorInfo) -> Result<(), String> {
    let window = orb_window(app)?;
    // 显影同样禁用过渡：透明窗口淡入时实时桌面会透出渐显的底图。
    platform::suppress_transitions(&window, || {
        let mut ops = WebviewOps(&window);
        ops.set_always_on_top(true)?;
        crate::cmd::orb::apply_orb_click_through(app, false)?;
        apply_geometry(
            &mut ops,
            PhysicalPosition::new(monitor.x, monitor.y),
            PhysicalSize::new(monitor.width, monitor.height),
        )?;
        ops.set_visible(true)
    })?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 退出截屏：还原现场并清标志；还原失败也要先清标志，避免 Orb 永久冻结。
pub fn exit(app: &AppHandle, snapshot: OrbWindowSnapshot) -> Result<(), String> {
    let result = match orb_window(app) {
        Ok(window) => restore(app, &window, snapshot),
        Err(error) => Err(error),
    };
    set_active(false);
    result
}

fn restore(
    app: &AppHandle,
    window: &WebviewWindow,
    snapshot: OrbWindowSnapshot,
) -> Result<(), String> {
    // 还原几何与可见性也禁用过渡：缩回小球时的动画同样会把实时桌面透出来。
    platform::suppress_transitions(window, || {
        restore_with(&mut WebviewOps(window), snapshot, |enabled| {
            crate::cmd::orb::apply_orb_click_through(app, enabled)
        })
    })
}

/// 异常残留复位：标志仍置位却没有会话时，按 OrbConf 把窗口拉回收起态。
pub fn recover_leftover(app: &AppHandle) {
    if !SCREENSHOT_ACTIVE.swap(false, Ordering::AcqRel) {
        return;
    }
    warn!("screenshot: cleared leftover active flag");
    let Some(window) = app.get_webview_window(WINDOW_ORB) else {
        return;
    };
    crate::window::apply_orb_window_attrs(app);
    let scale = window.scale_factor().unwrap_or(1.0);
    let edge = (ORB_COLLAPSED_WINDOW_SIZE as f64 * scale).round() as u32;
    let position = OrbConf::load()
        .ok()
        .and_then(|conf| match (conf.position_x, conf.position_y) {
            (Some(x), Some(y)) => Some(PhysicalPosition::new(x, y)),
            _ => None,
        });
    // 与 `restore` 同理：窗口可能正以全屏覆盖层形态停在原点，先隐藏再改几何并禁用过渡，
    // 否则缩回小球的那一帧会把小球画在左上角、实时桌面也会透过淡出动画。
    let result = platform::suppress_transitions(&window, || {
        reset_collapsed(
            &mut WebviewOps(&window),
            PhysicalSize::new(edge, edge),
            position,
        )
    });
    if let Err(error) = result {
        warn!("screenshot: leftover reset incomplete: {error}");
    }
}

/// 菜单项被点中、`popup_at` 刚返回时调用：立刻隐藏仍在关闭中的菜单窗口，
/// 并把「淡出起点」锚定在点击这一刻。截图链路随后才启动（枚举显示器、取光标、隐藏 Orb），
/// 若等到抓屏前才记时刻，动画往往已经播完或只记到结束时刻，剩余时长算成 0，动画帧照样被冻进底图。
pub(crate) use menu_fade::dismiss_menus_now;

/// 「菜单项被按下」这一刻的锚点，只读不消费：截图链路拿它当 t0 打分段耗时日志，
/// 用来判断真正的瓶颈在哪一段，而不是凭固定常量猜。
pub(super) use menu_fade::menu_anchor;
