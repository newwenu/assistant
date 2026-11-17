// 会话清理：取消、孤儿会话过期丢弃与异常残留复位。

use std::time::Duration;

use log::warn;
use tauri::{AppHandle, Manager};

use assistant_config::constant::{SCREENSHOT_SESSION_TTL_MS, WINDOW_ORB};

use super::orb_overlay;
use super::state::{discard_session, Phase, ScreenshotState};

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

/// 清理孤儿会话：宿主窗口已不在且超过 TTL 才丢弃，避免长时间占用内存原图。
pub(super) fn prune_stale(app: &AppHandle, state: &ScreenshotState) {
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
pub(super) fn reset_orphans(app: &AppHandle, state: &ScreenshotState) {
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
}
