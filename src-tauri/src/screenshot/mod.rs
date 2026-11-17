// 截屏提问会话状态机：Idle → Selecting → Cropping → Idle
// 单令牌会话；Orb 复用为覆盖层；底图与裁剪全程留在内存，只把字节归档进 AttachmentStore。

pub mod capture;
mod cleanup;
pub mod crop;
mod crop_command;
pub mod orb_overlay;
mod start;
mod state;

use tauri::AppHandle;

pub use capture::permission_granted;
pub use cleanup::{cancel, cancel_active};
pub use crop_command::crop;
pub use orb_overlay::is_active;
pub use start::start;
pub use state::ScreenshotState;

pub const NO_MONITOR: &str = "screenshot.err.no_monitor";
pub const CAPTURE_FAILED: &str = "screenshot.err.capture_failed";
pub const TOO_SMALL: &str = "screenshot.err.too_small";
pub const ENCODE_FAILED: &str = "screenshot.err.encode_failed";
pub const TOO_LARGE: &str = "screenshot.err.too_large";
pub const ARCHIVE_FAILED: &str = "screenshot.err.archive_failed";
pub const TOKEN_INVALID: &str = "screenshot.err.token_invalid";
pub const NO_SESSION: &str = "screenshot.err.no_session";
pub const PERMISSION_DENIED: &str = "screenshot.err.permission_denied";
pub const CANCEL_FAILED: &str = "screenshot.err.cancel_failed";

/// 只服务快捷键 / 右键菜单 / 托盘这类没有 invoke 返回值的入口。
pub fn emit_error(app: &AppHandle, code: &str) {
    crate::protocol::emit_orb_only(
        app,
        crate::protocol::event_name::SCREENSHOT_ERROR,
        code.to_string(),
    );
}
