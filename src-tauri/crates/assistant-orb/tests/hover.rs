use assistant_orb::hover::{
    pointer_inside_window, HoverState, PointerHoverObservation, HOVER_FADE_DURATION,
};
use std::time::Duration;

fn obs(enabled: bool, inside: bool, now_ms: u64, delay_ms: u64) -> PointerHoverObservation {
    PointerHoverObservation {
        enabled,
        delay: Duration::from_millis(delay_ms),
        pointer_inside: inside,
        now: Duration::from_millis(now_ms),
    }
}

#[test]
fn pointer_inside_boundary() {
    assert!(pointer_inside_window(0.0, 0.0, 0.0, 0.0, 100.0, 100.0));
    assert!(pointer_inside_window(99.999, 50.0, 0.0, 0.0, 100.0, 100.0));
    assert!(!pointer_inside_window(100.0, 50.0, 0.0, 0.0, 100.0, 100.0));
    assert!(!pointer_inside_window(50.0, 100.0, 0.0, 0.0, 100.0, 100.0));
}

#[test]
fn pointer_inside_rejects_non_finite() {
    assert!(!pointer_inside_window(
        f64::NAN,
        0.0,
        0.0,
        0.0,
        100.0,
        100.0
    ));
    assert!(!pointer_inside_window(
        f64::INFINITY,
        50.0,
        0.0,
        0.0,
        100.0,
        100.0
    ));
    assert!(!pointer_inside_window(
        f64::NEG_INFINITY,
        50.0,
        0.0,
        0.0,
        100.0,
        100.0
    ));
}

#[test]
fn pointer_inside_zero_size_window() {
    assert!(!pointer_inside_window(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
}

#[test]
fn pointer_inside_negative_origin() {
    assert!(pointer_inside_window(
        -50.0, -50.0, -100.0, -100.0, 200.0, 200.0
    ));
}

#[test]
fn visible_to_fading_out_on_delay() {
    assert_eq!(
        HoverState::Visible.update(&obs(true, true, 0, 0)),
        HoverState::FadingOut {
            elapsed: Duration::ZERO
        }
    );
    assert_eq!(
        HoverState::Visible.update(&obs(true, true, 299, 300)),
        HoverState::Visible
    );
    assert_eq!(
        HoverState::Visible.update(&obs(true, true, 300, 300)),
        HoverState::FadingOut {
            elapsed: Duration::ZERO
        }
    );
}

#[test]
fn fading_out_progression() {
    let state = HoverState::FadingOut {
        elapsed: Duration::from_millis(100),
    };
    assert_eq!(
        state.update(&obs(true, true, 50, 0)),
        HoverState::FadingOut {
            elapsed: Duration::from_millis(150)
        }
    );
    let state2 = HoverState::FadingOut {
        elapsed: Duration::ZERO,
    };
    assert_eq!(
        state2.update(&obs(true, true, HOVER_FADE_DURATION.as_millis() as u64, 0)),
        HoverState::Hidden
    );
}

#[test]
fn hidden_to_fading_in_on_pointer_leave() {
    assert_eq!(
        HoverState::Hidden.update(&obs(true, false, 0, 0)),
        HoverState::FadingIn {
            elapsed: Duration::ZERO
        }
    );
    assert_eq!(
        HoverState::Hidden.update(&obs(true, true, 0, 0)),
        HoverState::Hidden
    );
}

#[test]
fn fading_in_interrupted_by_pointer_return() {
    let state = HoverState::FadingIn {
        elapsed: Duration::from_millis(150),
    };
    assert_eq!(
        state.update(&obs(true, true, 0, 0)),
        HoverState::FadingOut {
            elapsed: Duration::from_millis(150)
        }
    );
    let state2 = HoverState::FadingIn {
        elapsed: Duration::ZERO,
    };
    assert_eq!(
        state2.update(&obs(true, true, 0, 0)),
        HoverState::FadingOut {
            elapsed: HOVER_FADE_DURATION
        }
    );
}

#[test]
fn fading_in_completes_to_visible() {
    let state = HoverState::FadingIn {
        elapsed: Duration::from_millis(200),
    };
    assert_eq!(state.update(&obs(true, false, 100, 0)), HoverState::Visible);
}

#[test]
fn disabled_always_yields_visible() {
    let disabled = obs(false, true, 0, 0);
    assert_eq!(HoverState::Visible.update(&disabled), HoverState::Visible);
    assert_eq!(HoverState::Hidden.update(&disabled), HoverState::Visible);
    assert_eq!(
        HoverState::FadingOut {
            elapsed: Duration::from_millis(100)
        }
        .update(&disabled),
        HoverState::Visible
    );
    assert_eq!(
        HoverState::FadingIn {
            elapsed: Duration::from_millis(100)
        }
        .update(&disabled),
        HoverState::Visible
    );
}

#[test]
fn opacity_values() {
    assert_eq!(HoverState::Visible.opacity(), 1.0);
    assert_eq!(HoverState::Hidden.opacity(), 0.0);
    assert!(
        (HoverState::FadingOut {
            elapsed: HOVER_FADE_DURATION
        }
        .opacity()
            - 0.0)
            .abs()
            < 0.001
    );
    assert!(
        (HoverState::FadingIn {
            elapsed: HOVER_FADE_DURATION
        }
        .opacity()
            - 1.0)
            .abs()
            < 0.001
    );
    assert!(
        (HoverState::FadingOut {
            elapsed: HOVER_FADE_DURATION / 2
        }
        .opacity()
            - 0.5)
            .abs()
            < 0.01
    );
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn pointer_inside_finite_coords(x in 0.0f64..100.0, y in 0.0f64..100.0) {
            assert_eq!(pointer_inside_window(x, y, 0.0, 0.0, 100.0, 100.0), x < 100.0 && y < 100.0);
        }

        #[test]
        fn opacity_always_in_range(elapsed_ms in 0u64..300, variant in 0u8..4) {
            let elapsed = Duration::from_millis(elapsed_ms.min(300));
            let opacity = match variant % 4 {
                0 => HoverState::Visible.opacity(),
                1 => HoverState::Hidden.opacity(),
                2 => HoverState::FadingOut { elapsed }.opacity(),
                _ => HoverState::FadingIn { elapsed }.opacity(),
            };
            assert!(opacity >= 0.0 && opacity <= 1.0);
        }

        #[test]
        fn disabled_always_visible(
            state_variant in 0u8..4,
            inside in proptest::bool::ANY,
            now_ms in 0u64..1000,
            delay_ms in 0u64..1000,
        ) {
            let state = match state_variant % 4 {
                0 => HoverState::Visible,
                1 => HoverState::FadingOut { elapsed: Duration::from_millis(100) },
                2 => HoverState::Hidden,
                _ => HoverState::FadingIn { elapsed: Duration::from_millis(100) },
            };
            let obs = PointerHoverObservation {
                enabled: false,
                delay: Duration::from_millis(delay_ms),
                pointer_inside: inside,
                now: Duration::from_millis(now_ms),
            };
            assert_eq!(state.update(&obs), HoverState::Visible);
        }

        #[test]
        fn opacity_monotonic_fading(e1 in 0u64..300, e2 in 0u64..300) {
            if e1 <= e2 {
                let o_out1 = HoverState::FadingOut { elapsed: Duration::from_millis(e1) }.opacity();
                let o_out2 = HoverState::FadingOut { elapsed: Duration::from_millis(e2) }.opacity();
                assert!(o_out1 >= o_out2);
                let o_in1 = HoverState::FadingIn { elapsed: Duration::from_millis(e1) }.opacity();
                let o_in2 = HoverState::FadingIn { elapsed: Duration::from_millis(e2) }.opacity();
                assert!(o_in1 <= o_in2);
            }
        }
    }
}

#[test]
fn full_cycle() {
    let enter = obs(true, true, 500, 0);
    let tick = obs(true, true, 100, 0);
    let leave = obs(true, false, 100, 0);
    let mut s = HoverState::Visible;
    s = s.update(&enter);
    for _ in 0..4 {
        s = s.update(&tick);
    }
    assert_eq!(s, HoverState::Hidden);
    s = s.update(&leave);
    for _ in 0..4 {
        s = s.update(&leave);
    }
    assert_eq!(s, HoverState::Visible);
}
