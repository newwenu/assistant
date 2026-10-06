#![cfg(feature = "slow-tests")]

use assistant_orb::hover::{
    pointer_inside_window, HoverState, PointerHoverObservation, HOVER_FADE_DURATION,
};
use proptest::prelude::*;
use std::time::Duration;

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
