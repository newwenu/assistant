// 参考: BongoCat/crates/bongocat-overlay/src/placement.rs — 位置约束逻辑
// 所有坐标统一使用物理像素（Physical pixels），与 Tauri API 保持一致

use std::time::Duration;

use log::error;
use tauri::{AppHandle, Manager};

use super::hover::ScreenBounds;

pub const PLACEMENT_SETTLE_DELAY: Duration = Duration::from_millis(1000);
pub const PLACEMENT_INSPECTION_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowBounds {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn clamp_to(self, screen: ScreenBounds) -> Self {
        let s_right = screen.x + screen.width as i32;
        let s_bottom = screen.y + screen.height as i32;
        let x = self.x.max(screen.x).min(s_right.saturating_sub(self.width as i32));
        let y = self.y.max(screen.y).min(s_bottom.saturating_sub(self.height as i32));
        Self { x, y, ..self }
    }
}

pub fn get_available_screens(app: &AppHandle) -> Vec<ScreenBounds> {
    let mut screens = Vec::new();
    match app.available_monitors() {
        Ok(monitors) => {
            for monitor in monitors {
                let pos = monitor.position();
                let size = monitor.size();
                screens.push(ScreenBounds {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
        Err(e) => {
            error!("Failed to get available monitors: {}", e);
        }
    }
    if screens.is_empty() {
        if let Ok(primary) = app.primary_monitor() {
            if let Some(monitor) = primary {
                let pos = monitor.position();
                let size = monitor.size();
                screens.push(ScreenBounds {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
    }
    screens
}

/// Whether every part of `bounds` lands on one of `screens`.
///
/// Uses a slice algorithm (from BongoCat) that correctly handles
/// multi-monitor L-shaped arrangements with gaps. A window that
/// straddles two adjacent displays is left alone as long as every
/// part of it still lands on a display.
pub fn bounds_inside_screens(screens: &[ScreenBounds], bounds: WindowBounds) -> bool {
    if screens.is_empty() {
        return false;
    }

    let (left, top, right, bottom) = window_edges(bounds);
    if right <= left || bottom <= top {
        return false;
    }

    let mut cuts = vec![left, right];
    for screen in screens {
        let (s_left, _, s_right, _) = screen_edges(*screen);
        for edge in [s_left, s_right] {
            if edge > left && edge < right {
                cuts.push(edge);
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut spans = Vec::with_capacity(screens.len());
    for slice in cuts.windows(2) {
        let (slice_left, slice_right) = (slice[0], slice[1]);
        spans.clear();
        for screen in screens {
            let (s_left, s_top, s_right, s_bottom) = screen_edges(*screen);
            if s_left <= slice_left && s_right >= slice_right {
                spans.push((s_top, s_bottom));
            }
        }
        if !spans_cover(&mut spans, (top, bottom)) {
            return false;
        }
    }
    true
}

fn spans_cover(spans: &mut [(i64, i64)], target: (i64, i64)) -> bool {
    spans.sort_unstable();
    let mut covered_until = target.0;
    for (start, end) in spans.iter().copied() {
        if end <= covered_until {
            continue;
        }
        if start > covered_until {
            return false;
        }
        covered_until = end;
        if covered_until >= target.1 {
            return true;
        }
    }
    covered_until >= target.1
}

fn window_edges(bounds: WindowBounds) -> (i64, i64, i64, i64) {
    let left = i64::from(bounds.x);
    let top = i64::from(bounds.y);
    (
        left,
        top,
        left + i64::from(bounds.width),
        top + i64::from(bounds.height),
    )
}

fn screen_edges(screen: ScreenBounds) -> (i64, i64, i64, i64) {
    let left = i64::from(screen.x);
    let top = i64::from(screen.y);
    (
        left,
        top,
        left + i64::from(screen.width),
        top + i64::from(screen.height),
    )
}

/// The display a window belongs to: largest overlap, then nearest center.
fn nearest_screen(screens: &[ScreenBounds], bounds: WindowBounds) -> Option<ScreenBounds> {
    let (left, top, right, bottom) = window_edges(bounds);
    let center_x = left.saturating_add(right) / 2;
    let center_y = top.saturating_add(bottom) / 2;

    let mut best: Option<(ScreenBounds, i64, i64)> = None;
    for screen in screens {
        let (s_left, s_top, s_right, s_bottom) = screen_edges(*screen);
        let overlap_x = right.min(s_right) - left.max(s_left);
        let overlap_y = bottom.min(s_bottom) - top.max(s_top);
        let overlap = overlap_x.max(0).saturating_mul(overlap_y.max(0));
        let s_center_x = s_left.saturating_add(s_right) / 2;
        let s_center_y = s_top.saturating_add(s_bottom) / 2;
        let distance = center_x
            .saturating_sub(s_center_x)
            .saturating_mul(center_x.saturating_sub(s_center_x))
            .saturating_add(
                center_y
                    .saturating_sub(s_center_y)
                    .saturating_mul(center_y.saturating_sub(s_center_y)),
            );
        let better = match best {
            None => true,
            Some((_, best_overlap, best_distance)) => {
                overlap > best_overlap || (overlap == best_overlap && distance < best_distance)
            }
        };
        if better {
            best = Some((*screen, overlap, distance));
        }
    }
    best.map(|(screen, _, _)| screen)
}

/// The box `bounds` should be moved to so that it lands fully on a display.
///
/// The target display is the one with the largest intersection with the window,
/// with ties broken by the nearest center. The window size is never changed.
pub fn correction_for_screens(screens: &[ScreenBounds], bounds: WindowBounds) -> Option<WindowBounds> {
    nearest_screen(screens, bounds).map(|screen| bounds.clamp_to(screen))
}

pub fn snap_to_nearest_screen(screens: &[ScreenBounds], bounds: WindowBounds) -> WindowBounds {
    if bounds_inside_screens(screens, bounds) {
        return bounds;
    }
    correction_for_screens(screens, bounds).unwrap_or(bounds)
}

pub fn check_and_snap_orb(app: &AppHandle, window_label: &str) -> Option<WindowBounds> {
    let window = app.get_webview_window(window_label)?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let bounds = WindowBounds::new(pos.x, pos.y, size.width, size.height);
    let screens = get_available_screens(app);
    if bounds_inside_screens(&screens, bounds) {
        return None;
    }
    let snapped = snap_to_nearest_screen(&screens, bounds);
    let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
    Some(snapped)
}