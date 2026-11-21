// Windows：枚举本进程的 `#32768` 顶层菜单窗口，按 DWM 合成帧排空关闭动画。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use log::info;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmFlush, DwmGetCompositionTimingInfo, DWM_TIMING_INFO};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, ShowWindow,
    SystemParametersInfoW, SPI_GETMENUANIMATION, SW_HIDE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

use assistant_config::constant::{
    SCREENSHOT_HIDE_SETTLE_MS, SCREENSHOT_MENU_FADE_CAP_MS, SCREENSHOT_MENU_FADE_FRAMES,
};

use crate::screenshot::CAPTURE_FAILED;

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
    let fresh = slot.is_some_and(|at| at.elapsed() <= Duration::from_millis(MENU_CLOSE_DEDUPE_MS));
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
pub(crate) fn menu_anchor() -> Option<Instant> {
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
            return Err(CAPTURE_FAILED.to_string());
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
pub(crate) fn dismiss_menus_now() {
    let hidden = hide_menu_windows(None).unwrap_or(false);
    record_menu_closed();
    log::trace!("screenshot: menu dismissed, hidden_windows={hidden}");
}

/// 抓屏前的兜底：托盘菜单这类没有「点中即隐藏」钩子的入口靠这一步清理残留菜单；
/// 预算内仍可见则返回 `capture_failed`，与 Orb 隐藏失败同一语义。
pub(crate) fn hide_leftover_menus() -> Result<(), String> {
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
pub(crate) fn settle_menu_fade() -> Result<(), String> {
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
