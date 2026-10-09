// 参考: BongoCat/crates/bongocat-overlay/src/hover.rs — 指针悬停隐藏逻辑

use std::time::Duration;

pub const HOVER_FADE_DURATION: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerHoverObservation {
    pub enabled: bool,
    pub delay: Duration,
    pub pointer_inside: bool,
    pub now: Duration,
}

pub fn pointer_inside_window(
    x: f64,
    y: f64,
    win_x: f64,
    win_y: f64,
    win_w: f64,
    win_h: f64,
) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    x >= win_x && x < win_x + win_w && y >= win_y && y < win_y + win_h
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HoverState {
    Visible,
    FadingOut { elapsed: Duration },
    Hidden,
    FadingIn { elapsed: Duration },
}

impl HoverState {
    pub fn update(self, obs: &PointerHoverObservation) -> Self {
        if !obs.enabled {
            return HoverState::Visible;
        }

        match self {
            HoverState::Visible => {
                if obs.pointer_inside && obs.now >= obs.delay {
                    HoverState::FadingOut {
                        elapsed: Duration::ZERO,
                    }
                } else {
                    HoverState::Visible
                }
            }
            HoverState::FadingOut { elapsed } => {
                let new_elapsed = elapsed + obs.now;
                if new_elapsed >= HOVER_FADE_DURATION {
                    HoverState::Hidden
                } else {
                    HoverState::FadingOut {
                        elapsed: new_elapsed,
                    }
                }
            }
            HoverState::Hidden => {
                if !obs.pointer_inside {
                    HoverState::FadingIn {
                        elapsed: Duration::ZERO,
                    }
                } else {
                    HoverState::Hidden
                }
            }
            HoverState::FadingIn { elapsed } => {
                if obs.pointer_inside {
                    HoverState::FadingOut {
                        elapsed: HOVER_FADE_DURATION - elapsed,
                    }
                } else {
                    let new_elapsed = elapsed + obs.now;
                    if new_elapsed >= HOVER_FADE_DURATION {
                        HoverState::Visible
                    } else {
                        HoverState::FadingIn {
                            elapsed: new_elapsed,
                        }
                    }
                }
            }
        }
    }

    pub fn opacity(self) -> f32 {
        let fade_secs = HOVER_FADE_DURATION.as_secs_f32();
        match self {
            HoverState::Visible => 1.0,
            HoverState::FadingOut { elapsed } => {
                if fade_secs > 0.0 {
                    1.0 - (elapsed.as_secs_f32() / fade_secs)
                } else {
                    0.0
                }
            }
            HoverState::Hidden => 0.0,
            HoverState::FadingIn { elapsed } => {
                if fade_secs > 0.0 {
                    elapsed.as_secs_f32() / fade_secs
                } else {
                    1.0
                }
            }
        }
    }

    pub fn should_toggle_click_through(self, prev: HoverState) -> Option<bool> {
        match (prev, self) {
            (HoverState::Visible, HoverState::FadingOut { .. }) => Some(true),
            (HoverState::FadingOut { .. }, HoverState::Hidden) => Some(true),
            (HoverState::Hidden, HoverState::FadingIn { .. }) => Some(false),
            (HoverState::FadingIn { .. }, HoverState::Visible) => Some(false),
            _ => None,
        }
    }
}
