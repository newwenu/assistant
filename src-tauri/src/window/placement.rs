// 参考: docs/store/I75-startup-layering-spec.md §9 #10 / #11 — Orb 定位与屏幕巡检

use tauri::Manager;

use assistant_config::constant::{
    ORB_CHROME_INSET, ORB_COLLAPSED_WINDOW_SIZE, ORB_HITBOX_SIZE, WINDOW_ORB,
};
use assistant_config::orb_conf::OrbConf;
use assistant_orb::placement;

/// 定位：保存位置优先（越界则吸附），否则主屏右下角。配置由调用点传入。
pub fn position_orb_window(handle: &tauri::AppHandle, conf: &OrbConf) {
    let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
        return;
    };

    let screens = placement::get_available_screens(handle);
    let scale = window.scale_factor().unwrap_or(1.0);
    let collapsed_phys = (ORB_COLLAPSED_WINDOW_SIZE as f64 * scale) as u32;

    if let (Some(x), Some(y)) = (conf.position_x, conf.position_y) {
        if let Some(snapped) = placement::ensure_orb_on_screen(
            &screens,
            x,
            y,
            collapsed_phys,
            collapsed_phys,
            ORB_CHROME_INSET,
            ORB_HITBOX_SIZE,
        ) {
            if let Err(e) = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y))
            {
                log::warn!("position orb window (saved) failed: {e}");
            }
        } else if let Err(e) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
            log::warn!("position orb window (saved fallback) failed: {e}");
        }
        return;
    }

    let area = placement::get_primary_work_area(handle).or_else(|| screens.first().copied());
    let Some(area) = area else {
        return;
    };
    let edge_margin = 20;
    let (x, y) = placement::anchor_bottom_right(&area, collapsed_phys, edge_margin);
    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
        log::warn!("position orb window (default) failed: {e}");
    }
}

/// 线程体每轮现读并回写 `OrbConf`：运行期配置以磁盘为准，不依赖启动快照。
pub fn spawn_screen_inspection(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(placement::PLACEMENT_INSPECTION_INTERVAL);
        if crate::lifecycle::should_exit() {
            break;
        }
        if crate::screenshot::is_active() {
            continue;
        }
        let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
            continue;
        };
        let Ok(pos) = window.outer_position() else {
            continue;
        };
        let Ok(size) = window.outer_size() else {
            continue;
        };
        let screens = placement::get_available_screens(&handle);
        let Some(snapped) = placement::ensure_orb_on_screen(
            &screens,
            pos.x,
            pos.y,
            size.width,
            size.height,
            ORB_CHROME_INSET,
            ORB_HITBOX_SIZE,
        ) else {
            continue;
        };
        if let Err(e) = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y)) {
            log::warn!("inspection: reposition orb window failed: {e}");
        }
        if let Ok(mut conf) = OrbConf::load() {
            conf.position_x = Some(snapped.x);
            conf.position_y = Some(snapped.y);
            if let Err(e) = conf.save() {
                log::warn!("inspection: save orb position failed: {e}");
            }
        }
    });
}
