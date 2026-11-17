// 截屏命令：全部只在 Orb 窗口内可用（第二道边界），令牌是第一道。

use tauri::{command, AppHandle, State, WebviewWindow};

use assistant_config::constant::WINDOW_ORB;

use crate::protocol::{ScreenshotCropArgsDto, ScreenshotCropResultDto, ScreenshotSessionDto};
use crate::screenshot::{self, ScreenshotState};

fn require_orb(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == WINDOW_ORB {
        Ok(())
    } else {
        Err(screenshot::NO_SESSION.to_string())
    }
}

#[command]
pub async fn start_screenshot(
    window: WebviewWindow,
    app: AppHandle,
    _state: State<'_, ScreenshotState>,
) -> Result<(), String> {
    require_orb(&window)?;
    screenshot::start(&app).await
}

#[command]
pub fn get_screenshot_session(
    window: WebviewWindow,
    state: State<'_, ScreenshotState>,
) -> Result<ScreenshotSessionDto, String> {
    require_orb(&window)?;
    state.session()
}

#[command]
pub async fn crop_screenshot(
    window: WebviewWindow,
    app: AppHandle,
    _state: State<'_, ScreenshotState>,
    args: ScreenshotCropArgsDto,
) -> Result<ScreenshotCropResultDto, String> {
    require_orb(&window)?;
    screenshot::crop(&app, args).await
}

#[command]
pub async fn cancel_screenshot(
    window: WebviewWindow,
    app: AppHandle,
    _state: State<'_, ScreenshotState>,
    token: String,
) -> Result<(), String> {
    require_orb(&window)?;
    screenshot::cancel(&app, &token).await
}

#[command]
pub fn screenshot_permission_granted(app: AppHandle) -> bool {
    screenshot::permission_granted(&app)
}
