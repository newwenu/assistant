// 截屏提问会话状态机：Idle → Selecting → Cropping → Idle
// 单令牌会话；Orb 复用为覆盖层；底图与裁剪全程留在内存，只把字节归档进 AttachmentStore。

pub mod capture;
pub mod crop;
pub mod orb_overlay;

use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{info, warn};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as AsyncMutex;

use assistant_config::constant::{
    SCREENSHOT_OVERLAY_SETTLE_MS, SCREENSHOT_SESSION_TTL_MS, WINDOW_ORB,
};

use crate::protocol::{ScreenshotCropArgsDto, ScreenshotCropResultDto, ScreenshotSessionDto};
use crate::screenshot::orb_overlay::OrbWindowSnapshot;

use capture::CaptureBackend;
use crop::{encode_crop, to_image_rect};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Capturing,
    Selecting,
    Cropping,
}

/// 抓屏目标：显示器几何 + 缩放比，只留在 Rust 侧做诊断。
#[derive(Clone, Copy, Debug)]
struct Target {
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f64,
}

struct PendingSession {
    token: String,
    preview: String,
    preview_width: u32,
    preview_height: u32,
    image: Arc<image::RgbaImage>,
    target: Target,
    created_at: Instant,
    orb: OrbWindowSnapshot,
}

struct Inner {
    phase: Phase,
    session: Option<PendingSession>,
}

pub struct ScreenshotState {
    op: AsyncMutex<()>,
    inner: std::sync::Mutex<Inner>,
    backend: Arc<dyn CaptureBackend>,
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

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
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

pub use orb_overlay::is_active;

pub fn permission_granted(app: &AppHandle) -> bool {
    capture::permission_granted(app)
}

/// 只服务快捷键 / 右键菜单 / 托盘这类没有 invoke 返回值的入口。
pub fn emit_error(app: &AppHandle, code: &str) {
    crate::protocol::emit_orb_only(
        app,
        crate::protocol::event_name::SCREENSHOT_ERROR,
        code.to_string(),
    );
}

/// 分段耗时：t0 取「菜单项被按下」那一刻（无菜单入口时退回本函数进入时刻），
/// 每段打一行 `stage=` 日志，用实测数据定位瓶颈，不靠固定常量猜。
struct StageTimer(Instant);

impl StageTimer {
    fn new() -> Self {
        Self(orb_overlay::menu_anchor().unwrap_or_else(Instant::now))
    }

    fn mark(&self, stage: &str) {
        info!(
            "screenshot: stage={stage} +{}ms",
            self.0.elapsed().as_millis()
        );
    }
}

/// 开始截屏：选屏 → 隐藏 Orb → 抓屏编码 → 建会话 → 下发 → 铺满显影。
pub async fn start(app: &AppHandle) -> Result<(), String> {
    let timer = StageTimer::new();
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    prune_stale(app, &state);
    reset_orphans(app, &state);

    {
        let inner = state.lock_inner();
        if inner.phase != Phase::Idle {
            info!("screenshot: start ignored, phase = {:?}", inner.phase);
            return Ok(());
        }
    }

    if !capture::permission_granted(app) {
        return Err(PERMISSION_DENIED.to_string());
    }

    // 菜单/托盘入口不经过前端工具栏，前端此刻仍是「小球形态」；而下面会把同一个窗口
    // 撑成全屏，小球 UI 的绝对定位会落到新窗口的原点（左上角）。先通知前端进入覆盖层
    // 模式隐藏球体，再往下走——后续的隐藏序列、菜单淡出排空与抓帧编码有足够时间让前端
    // 应用这个状态，等到 `present` 显影时窗口里已经是覆盖层而不是小球。
    crate::protocol::emit_orb_only(app, crate::protocol::event_name::SCREENSHOT_PREPARING, ());

    let backend = state.backend.clone();
    let monitors = tokio::task::spawn_blocking(move || backend.monitors())
        .await
        .map_err(|_| NO_MONITOR.to_string())?
        .map_err(|_| NO_MONITOR.to_string())?;
    timer.mark("monitors");

    let point = capture::cursor_point(app);
    let monitor =
        capture::select_monitor(&monitors, point).ok_or_else(|| NO_MONITOR.to_string())?;
    let scale_factor = capture::scale_factor_for(app, monitor);

    let snapshot = orb_overlay::begin(app)?;
    timer.mark("orb-hidden");

    {
        let mut inner = state.lock_inner();
        inner.phase = Phase::Capturing;
    }

    let captured = {
        let backend = state.backend.clone();
        tokio::task::spawn_blocking(move || {
            let image = capture::capture_monitor(backend.as_ref(), monitor)?;
            let preview = capture::encode_preview(&image)?;
            Ok::<_, String>((image, preview))
        })
        .await
        .map_err(|_| CAPTURE_FAILED.to_string())?
    };
    timer.mark("captured");

    let (image, preview) = match captured {
        Ok(pair) => pair,
        Err(error) => {
            let _ = orb_overlay::exit(app, snapshot);
            set_phase(&state, Phase::Idle);
            return Err(error);
        }
    };

    let target = Target {
        monitor_x: monitor.x,
        monitor_y: monitor.y,
        monitor_width: monitor.width,
        monitor_height: monitor.height,
        scale_factor,
    };
    info!(
        "screenshot: target {}x{} at ({}, {}), scale {:.2}, preview {}x{}",
        target.monitor_width,
        target.monitor_height,
        target.monitor_x,
        target.monitor_y,
        target.scale_factor,
        preview.width,
        preview.height
    );

    let token = uuid::Uuid::new_v4().simple().to_string();
    let session_dto = ScreenshotSessionDto {
        token: token.clone(),
        preview: preview.data_url.clone(),
        preview_width: preview.width,
        preview_height: preview.height,
    };

    {
        let mut inner = state.lock_inner();
        inner.session = Some(PendingSession {
            token,
            preview: preview.data_url,
            preview_width: preview.width,
            preview_height: preview.height,
            image: Arc::new(image),
            target,
            created_at: Instant::now(),
            orb: snapshot,
        });
        inner.phase = Phase::Selecting;
    }

    // 会话数据必须在 Orb 仍隐藏时下发，覆盖层一显影就有底图。
    crate::protocol::emit_orb_only(
        app,
        crate::protocol::event_name::SCREENSHOT_SESSION,
        session_dto,
    );

    if let Err(error) = orb_overlay::present(app, monitor) {
        let discarded = discard_session(&state);
        if let Some(session) = discarded {
            let _ = orb_overlay::exit(app, session.orb);
        }
        return Err(error);
    }
    timer.mark("presented");

    tokio::time::sleep(Duration::from_millis(SCREENSHOT_OVERLAY_SETTLE_MS)).await;
    Ok(())
}

/// 裁剪归档：选区 → 像素矩形 → 单次编码 → 归档 → 返回 attachmentId。
pub async fn crop(
    app: &AppHandle,
    args: ScreenshotCropArgsDto,
) -> Result<ScreenshotCropResultDto, String> {
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    let (image, orb_snapshot, target) = {
        let mut inner = state.lock_inner();
        if inner.phase != Phase::Selecting {
            return Err(TOKEN_INVALID.to_string());
        }
        let (image, orb, target) = match inner.session.as_ref() {
            Some(session) if session.token == args.token => {
                (session.image.clone(), session.orb, session.target)
            }
            _ => return Err(TOKEN_INVALID.to_string()),
        };
        inner.phase = Phase::Cropping;
        (image, orb, target)
    };
    info!(
        "screenshot: crop on {}x{} monitor at ({}, {}), scale {:.2}",
        target.monitor_width,
        target.monitor_height,
        target.monitor_x,
        target.monitor_y,
        target.scale_factor
    );

    let rect = match to_image_rect(
        args.x,
        args.y,
        args.width,
        args.height,
        image.width(),
        image.height(),
    ) {
        Some(rect) => rect,
        None => {
            restore_selecting(&state);
            return Err(TOO_SMALL.to_string());
        }
    };

    let encoded = tokio::task::spawn_blocking(move || encode_crop(image.as_ref(), rect))
        .await
        .map_err(|_| ENCODE_FAILED.to_string())?;

    let encoded = match encoded {
        Ok(encoded) => encoded,
        Err(error) => {
            restore_selecting(&state);
            return Err(error);
        }
    };

    let file_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), encoded.extension);
    let archived =
        crate::cmd::archive_bytes(app, &encoded.bytes, &file_name, encoded.media_type).await;

    let archived = match archived {
        Ok(archived) => archived,
        Err(error) => {
            warn!("screenshot: archive failed: {}", error);
            restore_selecting(&state);
            return Err(ARCHIVE_FAILED.to_string());
        }
    };

    // 归档成功后重新校验令牌再清会话，避免与取消/新会话交错。
    let cleared = {
        let mut inner = state.lock_inner();
        match inner.session.as_ref() {
            Some(session) if session.token == args.token => {
                inner.session = None;
                inner.phase = Phase::Idle;
                true
            }
            _ => false,
        }
    };
    if !cleared {
        return Err(TOKEN_INVALID.to_string());
    }

    let _ = orb_overlay::exit(app, orb_snapshot);

    Ok(ScreenshotCropResultDto {
        attachment_id: archived.attachment_id,
        file_name: archived.file_name,
        media_type: archived.media_type,
        bytes: archived.bytes,
        width: encoded.width,
        height: encoded.height,
    })
}

/// 取消：令牌匹配才生效，重复调用按幂等成功处理。
pub async fn cancel(app: &AppHandle, token: &str) -> Result<(), String> {
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    let matches = {
        let inner = state.lock_inner();
        matches!(inner.session.as_ref(), Some(session) if session.token == token)
    };
    if !matches {
        return Ok(());
    }

    let session = {
        let mut inner = state.lock_inner();
        inner.phase = Phase::Idle;
        inner.session.take()
    };

    if let Some(session) = session {
        let _ = orb_overlay::exit(app, session.orb);
    }
    Ok(())
}

/// 取消当前会话，不校验令牌；Orb 关闭请求等内部路径使用。
pub async fn cancel_active(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    let session = discard_session(&state);
    if let Some(session) = session {
        let _ = orb_overlay::exit(app, session.orb);
    }
    Ok(())
}

fn set_phase(state: &ScreenshotState, phase: Phase) {
    state.lock_inner().phase = phase;
}

fn discard_session(state: &ScreenshotState) -> Option<PendingSession> {
    let mut inner = state.lock_inner();
    inner.phase = Phase::Idle;
    inner.session.take()
}

fn restore_selecting(state: &ScreenshotState) {
    let mut inner = state.lock_inner();
    inner.phase = if inner.session.is_some() {
        Phase::Selecting
    } else {
        Phase::Idle
    };
}

/// 清理孤儿会话：宿主窗口已不在且超过 TTL 才丢弃，避免长时间占用内存原图。
fn prune_stale(app: &AppHandle, state: &ScreenshotState) {
    let has_host = app.get_webview_window(WINDOW_ORB).is_some();
    let stale = {
        let mut inner = state.lock_inner();
        let should_prune = match inner.session.as_ref() {
            Some(session) => should_prune(session.created_at.elapsed(), has_host),
            None => false,
        };
        if should_prune {
            inner.phase = Phase::Idle;
            inner.session.take()
        } else {
            None
        }
    };
    if stale.is_some() {
        warn!("screenshot: pruned orphan session");
    }
}

fn should_prune(elapsed: Duration, has_host: bool) -> bool {
    !has_host && elapsed >= Duration::from_millis(SCREENSHOT_SESSION_TTL_MS)
}

/// 复位异常残留：没有会话却停在非 Idle 相位，或标志仍置位（上次失败路径漏复位）。
fn reset_orphans(app: &AppHandle, state: &ScreenshotState) {
    let reset_phase = {
        let mut inner = state.lock_inner();
        if inner.session.is_none() && inner.phase != Phase::Idle {
            inner.phase = Phase::Idle;
            true
        } else {
            false
        }
    };
    if reset_phase {
        warn!("screenshot: reset orphan phase without session");
    }
    orb_overlay::recover_leftover(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prune_requires_missing_host_and_ttl() {
        let ttl = Duration::from_millis(SCREENSHOT_SESSION_TTL_MS);
        assert!(!should_prune(ttl, true));
        assert!(!should_prune(ttl - Duration::from_millis(1), false));
        assert!(should_prune(ttl, false));
    }

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
