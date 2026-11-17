// Windows：DWM 过渡禁用与 ShowWindow / DwmFlush 兜底。

use tauri::WebviewWindow;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
};
use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, ShowWindow, SW_HIDE};

use crate::screenshot::CAPTURE_FAILED;

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
pub(crate) fn suppress_transitions<R>(
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

pub(crate) fn force_hide(window: &WebviewWindow) -> Result<(), String> {
    let hwnd: HWND = window.hwnd().map_err(|e| e.to_string())?;
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    if unsafe { IsWindowVisible(hwnd).as_bool() } {
        return Err(CAPTURE_FAILED.to_string());
    }
    Ok(())
}

pub(crate) fn flush() -> Result<(), String> {
    unsafe {
        let _ = DwmFlush();
    }
    Ok(())
}
