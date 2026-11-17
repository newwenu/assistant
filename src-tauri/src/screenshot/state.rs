// 截屏会话状态：相位与待处理会话，只承载「会话当前是什么」这一职责。

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex as AsyncMutex;

use crate::protocol::ScreenshotSessionDto;

use super::capture::{self, CaptureBackend};
use super::orb_overlay::OrbWindowSnapshot;
use super::NO_SESSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Idle,
    Capturing,
    Selecting,
    Cropping,
}

/// 抓屏目标：显示器几何 + 缩放比，只留在 Rust 侧做诊断。
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub(super) monitor_x: i32,
    pub(super) monitor_y: i32,
    pub(super) monitor_width: u32,
    pub(super) monitor_height: u32,
    pub(super) scale_factor: f64,
}

pub(super) struct PendingSession {
    pub(super) token: String,
    pub(super) preview: String,
    pub(super) preview_width: u32,
    pub(super) preview_height: u32,
    pub(super) image: Arc<image::RgbaImage>,
    pub(super) target: Target,
    pub(super) created_at: Instant,
    pub(super) orb: OrbWindowSnapshot,
}

pub(super) struct Inner {
    pub(super) phase: Phase,
    pub(super) session: Option<PendingSession>,
}

pub struct ScreenshotState {
    pub(super) op: AsyncMutex<()>,
    pub(super) inner: std::sync::Mutex<Inner>,
    pub(super) backend: Arc<dyn CaptureBackend>,
}

impl ScreenshotState {
    pub fn new() -> Self {
        Self::with_backend(Arc::new(capture::XcapBackend))
    }

    pub fn with_backend(backend: Arc<dyn CaptureBackend>) -> Self {
        Self {
            op: AsyncMutex::new(()),
            inner: std::sync::Mutex::new(Inner {
                phase: Phase::Idle,
                session: None,
            }),
            backend,
        }
    }

    pub(super) fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// 覆盖层挂载后拉取会话；不在 Selecting 相位视为无会话。
    pub fn session(&self) -> Result<ScreenshotSessionDto, String> {
        let inner = self.lock_inner();
        match (&inner.phase, inner.session.as_ref()) {
            (Phase::Selecting, Some(session)) => Ok(ScreenshotSessionDto {
                token: session.token.clone(),
                preview: session.preview.clone(),
                preview_width: session.preview_width,
                preview_height: session.preview_height,
            }),
            _ => Err(NO_SESSION.to_string()),
        }
    }
}

impl Default for ScreenshotState {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn set_phase(state: &ScreenshotState, phase: Phase) {
    state.lock_inner().phase = phase;
}

pub(super) fn discard_session(state: &ScreenshotState) -> Option<PendingSession> {
    let mut inner = state.lock_inner();
    inner.phase = Phase::Idle;
    inner.session.take()
}

pub(super) fn restore_selecting(state: &ScreenshotState) {
    let mut inner = state.lock_inner();
    inner.phase = if inner.session.is_some() {
        Phase::Selecting
    } else {
        Phase::Idle
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_empty_in_idle_phase() {
        let state = ScreenshotState::new();
        assert_eq!(state.session().unwrap_err(), NO_SESSION);
    }

    #[test]
    fn session_snapshot_is_read_only_in_selecting_phase() {
        let state = ScreenshotState::new();
        {
            let mut inner = state.lock_inner();
            inner.phase = Phase::Selecting;
            inner.session = Some(PendingSession {
                token: "t".to_string(),
                preview: "data:image/jpeg;base64,".to_string(),
                preview_width: 10,
                preview_height: 10,
                image: Arc::new(image::RgbaImage::new(10, 10)),
                target: Target {
                    monitor_x: 0,
                    monitor_y: 0,
                    monitor_width: 10,
                    monitor_height: 10,
                    scale_factor: 1.0,
                },
                created_at: Instant::now(),
                orb: OrbWindowSnapshot {
                    position: tauri::PhysicalPosition::new(0, 0),
                    size: tauri::PhysicalSize::new(10, 10),
                    visible: true,
                    always_on_top: true,
                    click_through: false,
                },
            });
        }
        let dto = state.session().expect("session");
        assert_eq!(dto.token, "t");
        assert_eq!((dto.preview_width, dto.preview_height), (10, 10));
    }
}
