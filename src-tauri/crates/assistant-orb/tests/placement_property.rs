#![cfg(feature = "slow-tests")]

use assistant_orb::hover::ScreenBounds;
use assistant_orb::placement::{
    bounds_inside_screens, correction_for_screens, snap_orb_to_edge, WindowBounds,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn clamp_preserves_size(wx in -5000i32..5000, wy in -5000i32..5000, ww in 10u32..500, wh in 10u32..500) {
        let screen = ScreenBounds { x: 0, y: 0, width: 1920, height: 1080 };
        let clamped = WindowBounds::new(wx, wy, ww, wh).clamp_to(screen);
        assert_eq!((clamped.width, clamped.height), (ww, wh));
    }

    #[test]
    fn inside_screen_then_correction_is_noop(wx in 0i32..1720, wy in 0i32..880) {
        let screen = ScreenBounds { x: 0, y: 0, width: 1920, height: 1080 };
        let win = WindowBounds::new(wx, wy, 200, 200);
        assert!(bounds_inside_screens(&[screen], win));
        assert_eq!(correction_for_screens(&[screen], win).unwrap(), win);
    }

    #[test]
    fn snap_edge_preserves_size(wx in 0i32..1920, wy in 0i32..1080) {
        let screen = ScreenBounds { x: 0, y: 0, width: 1920, height: 1080 };
        let snapped = snap_orb_to_edge(&[screen], WindowBounds::new(wx, wy, 88, 88), 4, 0);
        assert_eq!((snapped.width, snapped.height), (88, 88));
    }
}