use super::hover::ScreenBounds;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowBounds {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn clamp_to(self, screen: ScreenBounds) -> Self {
        let s_right = screen.x + screen.width as i32;
        let s_bottom = screen.y + screen.height as i32;
        let x = self
            .x
            .max(screen.x)
            .min(s_right.saturating_sub(self.width as i32))
            .max(0);
        let y = self
            .y
            .max(screen.y)
            .min(s_bottom.saturating_sub(self.height as i32))
            .max(0);
        Self { x, y, ..self }
    }
}

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

/// 选出与 `bounds` 重叠面积最大、距离最近的屏幕；`bounds` 宽高为 0 时退化为「按中心点距离」。
pub fn nearest_screen(screens: &[ScreenBounds], bounds: WindowBounds) -> Option<ScreenBounds> {
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

pub fn correction_for_screens(
    screens: &[ScreenBounds],
    bounds: WindowBounds,
) -> Option<WindowBounds> {
    nearest_screen(screens, bounds).map(|screen| bounds.clamp_to(screen))
}

pub fn snap_to_nearest_screen(screens: &[ScreenBounds], bounds: WindowBounds) -> WindowBounds {
    if bounds_inside_screens(screens, bounds) {
        return bounds;
    }
    correction_for_screens(screens, bounds).unwrap_or(bounds)
}

pub fn snap_orb_to_edge(
    screens: &[ScreenBounds],
    bounds: WindowBounds,
    edge_buffer: i32,
    border_x: i32,
) -> WindowBounds {
    if screens.is_empty() {
        return bounds;
    }

    let corrected = if !bounds_inside_screens(screens, bounds) {
        snap_to_nearest_screen(screens, bounds)
    } else {
        bounds
    };

    let screen = match nearest_screen(screens, corrected) {
        Some(s) => s,
        None => return corrected,
    };

    let left_target_x = screen.x + edge_buffer;
    let right_target_x =
        screen.x + screen.width as i32 - corrected.width as i32 - edge_buffer + border_x;

    let dist_to_left = (corrected.x - left_target_x).abs();
    let dist_to_right = (corrected.x - right_target_x).abs();

    let target_x = if dist_to_left <= dist_to_right {
        left_target_x
    } else {
        right_target_x
    };

    let target_y = corrected
        .y
        .max(screen.y + edge_buffer)
        .min(screen.y + screen.height as i32 - corrected.height as i32 - edge_buffer);

    WindowBounds::new(target_x, target_y, corrected.width, corrected.height)
}
