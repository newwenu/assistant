// Orb 悬浮球复用为全屏覆盖层：隐藏序列 + 快照恢复
// 参考: Kiri ADR 0107 — 禁用 DWM 过渡动画 + 可见性校验 + ShowWindow 兜底 + DwmFlush

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use log::{info, warn};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use assistant_config::{
    constant::{ORB_COLLAPSED_WINDOW_SIZE, SCREENSHOT_HIDE_SETTLE_MS, WINDOW_ORB},
    orb_conf::OrbConf,
};

use super::capture::MonitorInfo;

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

/// 窗口操作的最小抽象：生产实现包住 `WebviewWindow`，单测注入假实现断言隐藏序列。
pub(crate) trait WindowOps {
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, String>;
    fn outer_size(&self) -> Result<PhysicalSize<u32>, String>;
    fn is_visible(&self) -> Result<bool, String>;
    fn set_visible(&mut self, visible: bool) -> Result<(), String>;
    fn force_hide(&mut self) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
    fn is_always_on_top(&self) -> Result<bool, String>;
    fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String>;
    fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String>;
    fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String>;
}

struct WebviewOps<'a>(&'a WebviewWindow);

impl WindowOps for WebviewOps<'_> {
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, String> {
        self.0.outer_position().map_err(|e| e.to_string())
    }

    fn outer_size(&self) -> Result<PhysicalSize<u32>, String> {
        self.0.outer_size().map_err(|e| e.to_string())
    }

    fn is_visible(&self) -> Result<bool, String> {
        self.0.is_visible().map_err(|e| e.to_string())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        if visible {
            self.0.show()
        } else {
            self.0.hide()
        }
        .map_err(|e| e.to_string())
    }

    fn force_hide(&mut self) -> Result<(), String> {
        platform::force_hide(self.0)
    }

    fn flush(&mut self) -> Result<(), String> {
        platform::flush()
    }

    fn is_always_on_top(&self) -> Result<bool, String> {
        self.0.is_always_on_top().map_err(|e| e.to_string())
    }

    fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String> {
        self.0
            .set_always_on_top(always_on_top)
            .map_err(|e| e.to_string())
    }

    fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String> {
        self.0.set_size(size).map_err(|e| e.to_string())
    }

    fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String> {
        self.0.set_position(position).map_err(|e| e.to_string())
    }
}

fn orb_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "screenshot.err.no_session".to_string())
}

fn snapshot_of(window: &WebviewWindow) -> Result<OrbWindowSnapshot, String> {
    snapshot_with(&WebviewOps(window), crate::cmd::orb::orb_click_through())
}

fn snapshot_with(ops: &dyn WindowOps, click_through: bool) -> Result<OrbWindowSnapshot, String> {
    Ok(OrbWindowSnapshot {
        position: ops.outer_position()?,
        size: ops.outer_size()?,
        visible: ops.is_visible()?,
        always_on_top: ops.is_always_on_top()?,
        click_through,
    })
}

/// 进入截屏：先置标志再隐藏，保证隐藏过程触发的窗口事件被守卫拦下。
pub fn begin(app: &AppHandle) -> Result<OrbWindowSnapshot, String> {
    let window = orb_window(app)?;
    let snapshot = snapshot_of(&window)?;
    set_active(true);
    if let Err(error) = hide_and_verify(&window) {
        set_active(false);
        return Err(error);
    }
    info!("screenshot: orb hidden");
    Ok(snapshot)
}

/// 几何写入：Tauri 2.12 的 `WebviewWindow` 没有 `set_bounds`，按规格 §8.5.1 的回退方案
/// 先写尺寸再写位置，恢复路径同样写回快照值而不是重算。
fn apply_geometry(
    ops: &mut dyn WindowOps,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    ops.set_size(size)?;
    ops.set_position(position)?;
    Ok(())
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

fn restore_with(
    ops: &mut dyn WindowOps,
    snapshot: OrbWindowSnapshot,
    click_through: impl FnOnce(bool) -> Result<(), String>,
) -> Result<(), String> {
    let mut issues: Vec<String> = Vec::new();

    // 必须在不可见时改几何：此刻窗口是可见的全屏覆盖层、位置在显示器原点，
    // `set_size` 与 `set_position` 是两次独立的主线程消息，中间会合成一帧——
    // 那一帧里窗口已缩回小球尺寸却仍在原点，于是左上角闪出一个可见的小球。
    if let Err(error) = ops.set_visible(false) {
        issues.push(format!("hide: {error}"));
    }

    if let Err(error) = ops.set_always_on_top(snapshot.always_on_top) {
        issues.push(format!("always_on_top: {error}"));
    }
    if let Err(error) = click_through(snapshot.click_through) {
        issues.push(format!("click_through: {error}"));
    }
    if let Err(error) = apply_geometry(ops, snapshot.position, snapshot.size) {
        issues.push(format!("bounds: {error}"));
    }

    // 只还原可见性，不抢焦点：用户当前前台窗口应保持焦点。
    if snapshot.visible {
        if let Err(error) = ops.set_visible(true) {
            issues.push(format!("visible: {error}"));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        warn!("screenshot: orb restore incomplete: {}", issues.join("; "));
        Err(super::CANCEL_FAILED.to_string())
    }
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
    // 与 `restore_with` 同理：窗口可能正以全屏覆盖层形态停在原点，先隐藏再改几何，
    // 否则缩回小球的那一帧会把小球画在左上角。
    if let Err(error) = window.hide() {
        warn!("screenshot: leftover hide failed: {}", error);
    }
    if let Err(error) = window.set_size(PhysicalSize::new(edge, edge)) {
        warn!("screenshot: leftover size restore failed: {}", error);
    }
    if let Ok(conf) = OrbConf::load() {
        if let (Some(x), Some(y)) = (conf.position_x, conf.position_y) {
            if let Err(error) = window.set_position(PhysicalPosition::new(x, y)) {
                warn!("screenshot: leftover position restore failed: {}", error);
            }
        }
    }
    if let Err(error) = window.show() {
        warn!("screenshot: leftover show failed: {}", error);
    }
}

/// 隐藏序列：`set_visible(false)` → 可见性仍为真时补平台兜底 → `flush`。
fn hide_sequence(ops: &mut dyn WindowOps) -> Result<(), String> {
    ops.set_visible(false)?;

    // `is_visible` 本身走主线程队列往返，返回的就是 hide 落地后的状态，不需要轮询等待。
    if ops.is_visible()? {
        ops.force_hide()?;
        if ops.is_visible()? {
            return Err(super::CAPTURE_FAILED.to_string());
        }
    }

    ops.flush()
}

/// 隐藏序列：先清掉本进程残留的弹出菜单，再在禁用 DWM 过渡的前提下隐藏 Orb，
/// 最后排空菜单关闭动画——抓屏读的是当前合成结果，动画未结束就会被冻进底图。
fn hide_and_verify(window: &WebviewWindow) -> Result<(), String> {
    platform::hide_leftover_menus()?;
    platform::suppress_transitions(window, || hide_sequence(&mut WebviewOps(window)))?;
    platform::settle_menu_fade()
}

/// 菜单项被点中、`popup_at` 刚返回时调用：立刻隐藏仍在关闭中的菜单窗口，
/// 并把「淡出起点」锚定在点击这一刻。截图链路随后才启动（枚举显示器、取光标、隐藏 Orb），
/// 若等到抓屏前才记时刻，动画往往已经播完或只记到结束时刻，剩余时长算成 0，动画帧照样被冻进底图。
pub(crate) fn dismiss_menus_now() {
    platform::dismiss_menus_now()
}

/// 「菜单项被按下」这一刻的锚点，只读不消费：截图链路拿它当 t0 打分段耗时日志，
/// 用来判断真正的瓶颈在哪一段，而不是凭固定常量猜。
pub(super) fn menu_anchor() -> Option<Instant> {
    platform::menu_anchor()
}

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use log::info;
    use tauri::WebviewWindow;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::Graphics::Dwm::{
        DwmFlush, DwmGetCompositionTimingInfo, DwmGetWindowAttribute, DwmSetWindowAttribute,
        DWMWA_TRANSITIONS_FORCEDISABLED, DWM_TIMING_INFO,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, ShowWindow,
        SystemParametersInfoW, SPI_GETMENUANIMATION, SW_HIDE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    use assistant_config::constant::{SCREENSHOT_MENU_FADE_CAP_MS, SCREENSHOT_MENU_FADE_FRAMES};

    use super::SCREENSHOT_HIDE_SETTLE_MS;

    /// Win32 弹出菜单的窗口类名。`TrackPopupMenu` 用它建一个**独立顶层窗口**，
    /// 既不是 Orb 的子窗口，也不受 `ShowWindow(orb)` 影响——Orb 右键菜单与托盘菜单都属这一类。
    const MENU_WINDOW_CLASS: &str = "#32768";
    /// 隐藏轮询间隔：菜单从「被点中」到「淡出结束并销毁」通常只有几十毫秒。
    const MENU_HIDE_POLL_MS: u64 = 5;
    /// 同一次菜单关闭会被先后观测两次（菜单事件回调 + `popup_at` 返回后的兜底），两次相隔
    /// 约 10~30ms。该窗口内的第二次观测视为同一次、保留更早的锚点；超过窗口则说明是上一次
    /// 交互留下的旧锚点（中间没截图），必须被新观测覆盖，否则本次等待会被算成 0。
    const MENU_CLOSE_DEDUPE_MS: u64 = 100;

    unsafe extern "system" fn collect_menu_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let found = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != unsafe { GetCurrentProcessId() } {
            return BOOL(1);
        }
        let mut class_name = [0u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
        if len <= 0 || String::from_utf16_lossy(&class_name[..len as usize]) != MENU_WINDOW_CLASS {
            return BOOL(1);
        }
        // 不过滤可见性：淡出动画期间窗口仍然存活但可能已不可见，这种「幽灵帧」才是最需要处理的。
        found.push(hwnd);
        BOOL(1)
    }

    /// 本进程当前存活的弹出菜单窗口（含关闭动画期间已不可见但尚未销毁的）。
    fn menu_windows() -> Vec<HWND> {
        let mut found: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(collect_menu_window),
                LPARAM(&mut found as *mut _ as isize),
            );
        }
        found
    }

    /// 最近一次观测到「菜单已关闭」的时刻。抓屏前从这一刻起补齐剩余淡出动画；
    /// 消费即清除，避免下一次与菜单无关的截图无谓等待。
    static MENU_CLOSED_AT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

    fn menu_closed_at() -> &'static Mutex<Option<Instant>> {
        MENU_CLOSED_AT.get_or_init(|| Mutex::new(None))
    }

    /// 记录关闭时刻。同一次菜单关闭会被先后观测两次（点击回调 + `popup_at` 返回后的兜底），
    /// 取其中**最早**的一次才能算足剩余动画时长；但超过去重窗口的旧锚点说明是上一次交互留下的，
    /// 必须被新观测覆盖。
    fn record_menu_closed() {
        let Ok(mut slot) = menu_closed_at().lock() else {
            return;
        };
        let fresh =
            slot.is_some_and(|at| at.elapsed() <= Duration::from_millis(MENU_CLOSE_DEDUPE_MS));
        if !fresh {
            *slot = Some(Instant::now());
        }
    }

    fn take_menu_closed() -> Option<Instant> {
        menu_closed_at()
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    /// 只读锚点、不消费：供截图链路把「按钮按下那一刻」当作 t0 打分段耗时日志，
    /// 真正的排空仍由 `settle_menu_fade` 取走。
    pub(super) fn menu_anchor() -> Option<Instant> {
        menu_closed_at().lock().ok().and_then(|slot| *slot)
    }

    /// 隐藏本进程存活的弹出菜单窗口，返回「本次是否碰到过菜单」。
    /// `deadline` 为 `None` 时只枚举一遍、不阻塞（菜单项被点中后的快速路径）；
    /// 为 `Some` 时轮询到「可见窗口全部隐藏」或超时（超时返回 `capture_failed`）。
    fn hide_menu_windows(deadline: Option<Instant>) -> Result<bool, String> {
        let mut seen = false;
        loop {
            let menus = menu_windows();
            if menus.is_empty() {
                return Ok(seen);
            }
            seen = true;
            let mut visible_left = false;
            for hwnd in &menus {
                if !unsafe { IsWindowVisible(*hwnd).as_bool() } {
                    continue;
                }
                visible_left = true;
                unsafe {
                    let _ = ShowWindow(*hwnd, SW_HIDE);
                }
            }
            // 已不可见（正在淡出）不再轮询：剩下的动画帧由 `settle_menu_fade` 排空。
            if !visible_left {
                return Ok(seen);
            }
            let Some(deadline) = deadline else {
                return Ok(seen);
            };
            if Instant::now() >= deadline {
                return Err(super::super::CAPTURE_FAILED.to_string());
            }
            std::thread::sleep(Duration::from_millis(MENU_HIDE_POLL_MS));
        }
    }

    /// 菜单项被点中时调用（菜单事件回调第一行，早于 `popup_at` 返回）：把淡出起点锚定在
    /// 「点击这一刻」，并顺带隐藏仍存活的菜单窗口。实测 `TrackPopupMenu` 返回时菜单窗口已被
    /// 销毁（`IsWindow` 为假），`SW_HIDE` 通常来不及生效，真正起作用的只有时间锚点。
    /// 锚定**无条件**执行：DWM 的关闭动画由合成器用缓存表面自行播完，窗口枚举不到并不代表
    /// 屏幕上没有残留帧。截图链路随后才启动（枚举显示器、取光标、隐藏 Orb），若等到抓屏前
    /// 才记时刻，剩余时长会被算成 0。
    pub(super) fn dismiss_menus_now() {
        let hidden = hide_menu_windows(None).unwrap_or(false);
        record_menu_closed();
        log::debug!("screenshot: menu dismissed, hidden_windows={hidden}");
    }

    /// 抓屏前的兜底：托盘菜单这类没有「点中即隐藏」钩子的入口靠这一步清理残留菜单；
    /// 预算内仍可见则返回 `capture_failed`，与 Orb 隐藏失败同一语义。
    pub(super) fn hide_leftover_menus() -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_millis(SCREENSHOT_HIDE_SETTLE_MS);
        if hide_menu_windows(Some(deadline))? {
            record_menu_closed();
        }
        Ok(())
    }

    /// 抓屏前排空菜单淡出：`ShowWindow(SW_HIDE)` 只让窗口不再可见，已经在播的关闭动画仍会
    /// 继续合成，而 `BitBlt(GetDesktopWindow)` 读的是当前合成结果，会把半透明菜单帧冻进底图。
    /// 实测淡出是 19 个 DWM 合成帧的线性过渡，因此这里按帧排空而不是按毫秒 sleep：
    /// `DwmFlush` 即使空闲也会等到下一个合成帧，连打 N 次即恰好消费 N 帧动画，
    /// 与显示器刷新率解耦。系统关掉菜单动画时没有淡出，只做合成刷新。
    pub(super) fn settle_menu_fade() -> Result<(), String> {
        if let Some(at) = take_menu_closed() {
            let age = at.elapsed();
            let animation = menu_animation_enabled();
            let flushed = if animation {
                drain_menu_fade_frames(age)
            } else {
                0
            };
            info!(
                "screenshot: menu fade anchor={}ms period={}ms flushed_frames={} animation={animation} live_windows={}",
                age.as_millis(),
                frame_period().as_millis(),
                flushed,
                menu_windows().len()
            );
        }
        // 排空循环的最后一个 `DwmFlush` 已经等到动画之后的合成帧；无锚点时 Orb 的隐藏
        // 序列已自带一次 flush，这里不再补刷，避免重复消费帧。
        Ok(())
    }

    /// 从「菜单关闭」这一刻起，把还没播完的淡出帧用 `DwmFlush` 一帧一帧消费掉，返回实际
    /// 消费的帧数。锚点之后已经过去的时间按帧周期折算成已消费帧数，避免重复等待。
    fn drain_menu_fade_frames(age: Duration) -> u32 {
        let period = frame_period();
        let already = (age.as_micros() / period.as_micros().max(1)) as u32;
        let remaining = SCREENSHOT_MENU_FADE_FRAMES.saturating_sub(already);
        let started = Instant::now();
        let mut flushed = 0u32;
        for _ in 0..remaining {
            unsafe {
                let _ = DwmFlush();
            }
            flushed += 1;
            // 合成器异常（远程桌面、窗口被遮挡）时 `DwmFlush` 可能提前返回，防止忙等。
            if started.elapsed() >= Duration::from_millis(SCREENSHOT_MENU_FADE_CAP_MS) {
                break;
            }
        }
        flushed
    }

    /// 当前合成帧周期。`DwmGetCompositionTimingInfo` 直接回报刷新率，不需要像 `DwmFlush`
    /// 那样靠消费帧来测——后者会在关键路径上白等两帧。查询失败时按 60Hz 兜底。
    fn frame_period() -> Duration {
        let mut timing = DWM_TIMING_INFO {
            cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
            ..Default::default()
        };
        let queried = unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing).is_ok() };
        let ratio = timing.rateRefresh;
        let (numerator, denominator) = (ratio.uiNumerator, ratio.uiDenominator);
        if queried && numerator > 0 && denominator > 0 {
            Duration::from_micros(1_000_000u64 * denominator as u64 / numerator as u64)
        } else {
            Duration::from_micros(16_667)
        }
    }

    /// 系统「菜单动画」开关：关掉时没有淡出，无需等待。
    fn menu_animation_enabled() -> bool {
        let mut enabled = 0u32;
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETMENUANIMATION,
                0,
                Some(&mut enabled as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        enabled != 0
    }

    /// 过渡禁用作用域：进入时禁用，离开时写回读取到的原值（不是无条件「启用」）。
    struct TransitionScope {
        hwnd: HWND,
        original: windows::core::BOOL,
    }

    impl Drop for TransitionScope {
        fn drop(&mut self) {
            unsafe {
                let _ = DwmSetWindowAttribute(
                    self.hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED,
                    &self.original as *const _ as *const _,
                    std::mem::size_of_val(&self.original) as u32,
                );
            }
        }
    }

    /// 在禁用 DWM 过渡的前提下执行窗口操作：`hide` / `show` / 改几何都会走过渡动画，
    /// Orb 是透明分层窗口，动画期间会把实时桌面透出来，与「冻结底图」冲突。
    pub(super) fn suppress_transitions<R>(
        window: &WebviewWindow,
        run: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        let hwnd: HWND = window.hwnd().map_err(|e| e.to_string())?;

        // 读失败时保留初始值 0（过渡启用），与系统默认一致，避免把窗口永久留在禁用态。
        let mut original = windows::core::BOOL(0);
        let _ = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &mut original as *mut _ as *mut _,
                std::mem::size_of_val(&original) as u32,
            )
        };

        let disabled = windows::core::BOOL(1);
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &disabled as *const _ as *const _,
                std::mem::size_of_val(&disabled) as u32,
            )
            .map_err(|e| e.to_string())?;
        }
        let _scope = TransitionScope { hwnd, original };

        run()
    }

    pub(super) fn force_hide(window: &WebviewWindow) -> Result<(), String> {
        let hwnd: HWND = window.hwnd().map_err(|e| e.to_string())?;
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        if unsafe { IsWindowVisible(hwnd).as_bool() } {
            return Err(super::super::CAPTURE_FAILED.to_string());
        }
        Ok(())
    }

    pub(super) fn flush() -> Result<(), String> {
        unsafe {
            let _ = DwmFlush();
        }
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use std::time::Instant;

    use super::WebviewWindow;

    /// 非 Windows 平台没有 `#32768` 顶层菜单窗口，清理与排空都是空实现。
    pub(super) fn dismiss_menus_now() {}

    pub(super) fn hide_leftover_menus() -> Result<(), String> {
        Ok(())
    }

    pub(super) fn settle_menu_fade() -> Result<(), String> {
        Ok(())
    }

    pub(super) fn suppress_transitions<R>(
        _window: &WebviewWindow,
        run: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        run()
    }

    pub(super) fn force_hide(_window: &WebviewWindow) -> Result<(), String> {
        Ok(())
    }

    pub(super) fn flush() -> Result<(), String> {
        Ok(())
    }

    pub(super) fn menu_anchor() -> Option<Instant> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeOps {
        visible: bool,
        always_on_top: bool,
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        hide_takes_effect: bool,
        force_hide_takes_effect: bool,
        fail_always_on_top: bool,
        calls: RefCell<Vec<&'static str>>,
    }

    impl FakeOps {
        fn new() -> Self {
            Self {
                visible: true,
                always_on_top: false,
                position: PhysicalPosition::new(100, 200),
                size: PhysicalSize::new(88, 88),
                hide_takes_effect: true,
                force_hide_takes_effect: true,
                fail_always_on_top: false,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }

    impl WindowOps for FakeOps {
        fn outer_position(&self) -> Result<PhysicalPosition<i32>, String> {
            Ok(self.position)
        }

        fn outer_size(&self) -> Result<PhysicalSize<u32>, String> {
            Ok(self.size)
        }

        fn is_visible(&self) -> Result<bool, String> {
            Ok(self.visible)
        }

        fn set_visible(&mut self, visible: bool) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push(if visible { "show" } else { "hide" });
            if visible || self.hide_takes_effect {
                self.visible = visible;
            }
            Ok(())
        }

        fn force_hide(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("force_hide");
            if self.force_hide_takes_effect {
                self.visible = false;
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("flush");
            Ok(())
        }

        fn is_always_on_top(&self) -> Result<bool, String> {
            Ok(self.always_on_top)
        }

        fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String> {
            self.calls.borrow_mut().push("always_on_top");
            if self.fail_always_on_top {
                return Err("boom".to_string());
            }
            self.always_on_top = always_on_top;
            Ok(())
        }

        fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String> {
            self.calls.borrow_mut().push("size");
            self.size = size;
            Ok(())
        }

        fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String> {
            self.calls.borrow_mut().push("position");
            self.position = position;
            Ok(())
        }
    }

    fn snapshot(visible: bool) -> OrbWindowSnapshot {
        OrbWindowSnapshot {
            position: PhysicalPosition::new(10, 20),
            size: PhysicalSize::new(88, 88),
            visible,
            always_on_top: true,
            click_through: true,
        }
    }

    #[test]
    fn hide_sequence_hides_then_flushes() {
        let mut ops = FakeOps::new();
        hide_sequence(&mut ops).expect("hide");
        assert_eq!(ops.calls(), vec!["hide", "flush"]);
        assert!(!ops.visible);
    }

    #[test]
    fn hide_sequence_falls_back_to_force_hide() {
        let mut ops = FakeOps::new();
        ops.hide_takes_effect = false;
        hide_sequence(&mut ops).expect("force hide fallback");
        assert_eq!(ops.calls(), vec!["hide", "force_hide", "flush"]);
        assert!(!ops.visible);
    }

    #[test]
    fn hide_sequence_reports_capture_failed() {
        let mut ops = FakeOps::new();
        ops.hide_takes_effect = false;
        ops.force_hide_takes_effect = false;
        assert_eq!(
            hide_sequence(&mut ops).unwrap_err(),
            crate::screenshot::CAPTURE_FAILED
        );
        assert_eq!(ops.calls(), vec!["hide", "force_hide"]);
    }

    #[test]
    fn snapshot_reads_all_window_fields() {
        let ops = FakeOps::new();
        let snap = snapshot_with(&ops, true).expect("snapshot");
        assert_eq!(snap.position, PhysicalPosition::new(100, 200));
        assert_eq!(snap.size, PhysicalSize::new(88, 88));
        assert!(snap.visible);
        assert!(!snap.always_on_top);
        assert!(snap.click_through);
    }

    #[test]
    fn restore_writes_snapshot_and_click_through() {
        let mut ops = FakeOps::new();
        let mut restored_click_through = None;
        restore_with(&mut ops, snapshot(false), |enabled| {
            restored_click_through = Some(enabled);
            Result::<(), String>::Ok(())
        })
        .expect("restore");

        assert_eq!(restored_click_through, Some(true));
        assert!(ops.always_on_top);
        assert_eq!(ops.position, PhysicalPosition::new(10, 20));
        assert_eq!(ops.size, PhysicalSize::new(88, 88));
        assert!(!ops.visible);
        // 快照本就不可见时不再补一次 show。
        assert_eq!(
            ops.calls(),
            vec!["hide", "always_on_top", "size", "position"]
        );
    }

    /// 可见的全屏覆盖层缩回小球时，几何必须已在隐藏状态下改完，否则中间帧会把
    /// 小球画在原点（左上角）。这里直接断言「hide 早于 size 与 position」。
    #[test]
    fn restore_hides_before_geometry_change() {
        let mut ops = FakeOps::new();
        ops.size = PhysicalSize::new(2520, 1680);
        ops.position = PhysicalPosition::new(0, 0);
        restore_with(&mut ops, snapshot(true), |_| Ok(())).expect("restore");

        let calls = ops.calls();
        let hide_at = calls.iter().position(|c| *c == "hide").expect("hide");
        let size_at = calls.iter().position(|c| *c == "size").expect("size");
        let position_at = calls
            .iter()
            .position(|c| *c == "position")
            .expect("position");
        assert!(hide_at < size_at, "hide must precede size: {calls:?}");
        assert!(
            hide_at < position_at,
            "hide must precede position: {calls:?}"
        );
        assert_eq!(calls.last(), Some(&"show"));
        assert_eq!(ops.size, PhysicalSize::new(88, 88));
        assert_eq!(ops.position, PhysicalPosition::new(10, 20));
        assert!(ops.visible);
    }

    #[test]
    fn restore_keeps_going_and_reports_cancel_failed() {
        let mut ops = FakeOps::new();
        ops.fail_always_on_top = true;
        assert_eq!(
            restore_with(&mut ops, snapshot(true), |_| Ok(())).unwrap_err(),
            crate::screenshot::CANCEL_FAILED
        );
        assert_eq!(
            ops.calls(),
            vec!["hide", "always_on_top", "size", "position", "show"]
        );
        assert!(ops.visible);
    }
}
