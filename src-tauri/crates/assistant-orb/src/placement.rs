// 参考: BongoCat/crates/bongocat-overlay/src/placement.rs — 位置约束逻辑

use std::time::Duration;

pub const PLACEMENT_SETTLE_DELAY: Duration = Duration::from_millis(1000);
pub const PLACEMENT_INSPECTION_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn bounds_inside_screens(screens: &[ScreenBounds], bounds: WindowBounds) -> bool {
    if screens.is_empty() {
        return false;
    }

    let left = bounds.x;
    let top = bounds.y;
    let right = bounds.x + bounds.width as i32;
    let bottom = bounds.y + bounds.height as i32;

    if right <= left || bottom <= top {
        return false;
    }

    for screen in screens {
        let s_left = screen.x;
        let s_top = screen.y;
        let s_right = screen.x + screen.width as i32;
        let s_bottom = screen.y + screen.height as i32;

        let overlap_left = left.max(s_left);
        let overlap_top = top.max(s_top);
        let overlap_right = right.min(s_right);
        let overlap_bottom = bottom.min(s_bottom);

        if overlap_right > overlap_left && overlap_bottom > overlap_top {
            return true;
        }
    }

    false
}

pub fn snap_to_nearest_screen(screens: &[ScreenBounds], bounds: WindowBounds) -> WindowBounds {
    if bounds_inside_screens(screens, bounds) {
        return bounds;
    }

    let center_x = bounds.x + bounds.width as i32 / 2;
    let center_y = bounds.y + bounds.height as i32 / 2;

    let mut best_screen = screens.first().copied();
    let mut best_dist = i64::MAX;

    for screen in screens {
        let s_center_x = screen.x + screen.width as i32 / 2;
        let s_center_y = screen.y + screen.height as i32 / 2;
        let dist = (center_x - s_center_x).abs() as i64 + (center_y - s_center_y).abs() as i64;
        if dist < best_dist {
            best_dist = dist;
            best_screen = Some(*screen);
        }
    }

    if let Some(screen) = best_screen {
        let new_x = (screen.x + screen.width as i32 / 2 - bounds.width as i32 / 2)
            .max(screen.x)
            .min(screen.x + screen.width as i32 - bounds.width as i32);
        let new_y = (screen.y + screen.height as i32 / 2 - bounds.height as i32 / 2)
            .max(screen.y)
            .min(screen.y + screen.height as i32 - bounds.height as i32);
        WindowBounds {
            x: new_x,
            y: new_y,
            ..bounds
        }
    } else {
        bounds
    }
}