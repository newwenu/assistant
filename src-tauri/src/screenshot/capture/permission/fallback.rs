// 非 macOS 平台：抓屏不需要系统级授权，前置校验恒为通过。

use tauri::AppHandle;

pub fn permission_granted(_app: &AppHandle) -> bool {
    true
}
