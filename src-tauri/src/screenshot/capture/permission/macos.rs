// macOS：`CGPreflightScreenCaptureAccess` 预检 + 实测枚举校验，成功缓存、失败短间隔重试。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::super::{CaptureBackend, XcapBackend};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> u8;
}

const RETRY_INTERVAL: Duration = Duration::from_secs(3);

static GRANTED: AtomicBool = AtomicBool::new(false);
static LAST_CHECK: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// 预检 + 实测校验：预检通过仍要枚举一次显示器，因为 TCC 授权可能陈旧
/// （预检通过但实际抓不到）；成功即缓存，失败按短间隔重试。
pub fn permission_granted(_app: &AppHandle) -> bool {
    if GRANTED.load(Ordering::Acquire) {
        return true;
    }

    let mut last = LAST_CHECK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((checked_at, ok)) = *last {
        if !ok && checked_at.elapsed() < RETRY_INTERVAL {
            return false;
        }
    }

    let preflight = unsafe { CGPreflightScreenCaptureAccess() != 0 };
    let usable = preflight
        && XcapBackend
            .monitors()
            .map(|monitors| !monitors.is_empty())
            .unwrap_or(false);

    if usable {
        GRANTED.store(true, Ordering::Release);
    }
    *last = Some((Instant::now(), usable));
    usable
}
