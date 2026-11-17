// 非 Windows 平台：没有 DWM 合成器过渡，隐藏与刷新都无需平台兜底。

use tauri::WebviewWindow;

pub(crate) fn suppress_transitions<R>(
    _window: &WebviewWindow,
    run: impl FnOnce() -> Result<R, String>,
) -> Result<R, String> {
    run()
}

pub(crate) fn force_hide(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

pub(crate) fn flush() -> Result<(), String> {
    Ok(())
}
