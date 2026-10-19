use assistant_orb::hover::ScreenBounds;
use assistant_orb::placement::{
    bounds_inside_screens, correction_for_screens, snap_orb_to_edge, snap_to_nearest_screen,
    WindowBounds,
};

fn screen(x: i32, y: i32, w: u32, h: u32) -> ScreenBounds {
    ScreenBounds {
        x,
        y,
        width: w,
        height: h,
    }
}

#[test]
fn bounds_inside_single_screen() {
    let s = screen(0, 0, 1920, 1080);
    assert!(bounds_inside_screens(
        &[s],
        WindowBounds::new(100, 100, 200, 200)
    ));
    assert!(!bounds_inside_screens(
        &[s],
        WindowBounds::new(1850, 100, 200, 200)
    ));
    assert!(!bounds_inside_screens(
        &[s],
        WindowBounds::new(0, 0, 1921, 1080)
    ));
    assert!(!bounds_inside_screens(
        &[],
        WindowBounds::new(0, 0, 100, 100)
    ));
}

#[test]
fn bounds_inside_multi_monitor() {
    let s1 = screen(0, 0, 1920, 1080);
    let s2 = screen(1920, 0, 1920, 1080);
    assert!(bounds_inside_screens(
        &[s1, s2],
        WindowBounds::new(1820, 100, 200, 200)
    ));
    let s3 = screen(0, 1080, 1920, 1080);
    assert!(!bounds_inside_screens(
        &[s1, s2, s3],
        WindowBounds::new(1800, 1000, 200, 200)
    ));
}

#[test]
fn bounds_inside_gap_between_screens() {
    let s1 = screen(0, 0, 960, 1080);
    let s2 = screen(1060, 0, 960, 1080);
    assert!(!bounds_inside_screens(
        &[s1, s2],
        WindowBounds::new(950, 100, 120, 200)
    ));
}

#[test]
fn clamp_to_constrains_and_preserves_size() {
    let s = screen(0, 0, 1920, 1080);
    let win = WindowBounds::new(100, 100, 200, 200);
    assert_eq!(win.clamp_to(s), win);
    let far = WindowBounds::new(5000, 5000, 100, 100);
    let clamped = far.clamp_to(s);
    assert_eq!(clamped.width, 100);
    assert!(clamped.x + 100 <= 1920);
    assert!(clamped.y + 100 <= 1080);
}

#[test]
fn clamp_far_negative_to_origin() {
    let s = screen(0, 0, 1920, 1080);
    let clamped = WindowBounds::new(-10000, -10000, 100, 100).clamp_to(s);
    assert_eq!((clamped.x, clamped.y), (0, 0));
}

#[test]
fn clamp_oversized_window_should_stay_non_negative() {
    let s = screen(0, 0, 1920, 1080);
    let clamped = WindowBounds::new(0, 0, 2560, 1440).clamp_to(s);
    assert!(clamped.x >= 0);
    assert!(clamped.y >= 0);
}

#[test]
fn correction_moves_onto_screen() {
    let s = screen(0, 0, 1920, 1080);
    let corrected = correction_for_screens(&[s], WindowBounds::new(2000, 2000, 100, 100)).unwrap();
    assert!(bounds_inside_screens(&[s], corrected));
    assert!(correction_for_screens(&[], WindowBounds::new(0, 0, 100, 100)).is_none());
}

#[test]
fn snap_noop_when_inside() {
    let s = screen(0, 0, 1920, 1080);
    let win = WindowBounds::new(100, 100, 200, 200);
    assert_eq!(snap_to_nearest_screen(&[s], win), win);
}

#[test]
fn snap_orb_edge() {
    let s = screen(0, 0, 1920, 1080);
    assert_eq!(
        snap_orb_to_edge(&[s], WindowBounds::new(100, 100, 88, 88), 4, 0).x,
        4
    );
    assert_eq!(
        snap_orb_to_edge(&[s], WindowBounds::new(1800, 100, 88, 88), 4, 0).x,
        1920 - 88 - 4
    );
}

#[test]
fn zero_size_window_not_inside() {
    let s = screen(0, 0, 1920, 1080);
    assert!(!bounds_inside_screens(
        &[s],
        WindowBounds::new(100, 100, 0, 0)
    ));
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
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
            let snapped = snap_orb_to_edge(&[screen], WindowBounds::new(wx, wy, 100, 100), 4, 0);
            assert_eq!((snapped.width, snapped.height), (88, 88));
        }
    }
}
