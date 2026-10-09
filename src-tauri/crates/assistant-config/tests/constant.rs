use assistant_config::constant::*;

#[test]
fn window_geometry_consistent() {
    assert_eq!(
        ORB_COLLAPSED_WINDOW_SIZE,
        ORB_COLLAPSED_SIZE + 2 * ORB_CHROME_INSET
    );
    assert_eq!(
        ORB_EXPANDED_WINDOW_WIDTH,
        ORB_EXPANDED_WIDTH + 2 * ORB_CHROME_INSET
    );
    assert_eq!(
        ORB_EXPANDED_WINDOW_HEIGHT,
        ORB_EXPANDED_HEIGHT + 2 * ORB_CHROME_INSET
    );
    let expected =
        (ORB_COLLAPSED_WINDOW_SIZE as i32 - ORB_CHROME_INSET as i32 - ORB_HITBOX_SIZE as i32)
            - ORB_CHROME_INSET as i32;
    assert_eq!(ORB_VISUAL_CENTER_OFFSET_X, expected);
}

#[test]
fn window_constraints_valid() {
    assert!(
        MAIN_WINDOW_MIN_WIDTH < MAIN_WINDOW_DEFAULT_WIDTH,
        "min width should be less than default"
    );
    assert!(
        MAIN_WINDOW_MIN_HEIGHT < MAIN_WINDOW_DEFAULT_HEIGHT,
        "min height should be less than default"
    );
    assert!(
        CHAT_INPUT_MIN_ROWS <= CHAT_INPUT_MAX_ROWS,
        "min rows should not exceed max rows"
    );
}

#[test]
fn identifiers_and_durations_valid() {
    assert!(!APP_IDENTIFIER.is_empty());
    assert!(!WINDOW_MAIN.is_empty());
    assert!(!WINDOW_ORB.is_empty());
    assert!(!WINDOW_SETTINGS.is_empty());
    assert_ne!(WINDOW_MAIN, WINDOW_ORB);
    assert_ne!(WINDOW_MAIN, WINDOW_SETTINGS);
    assert_ne!(WINDOW_ORB, WINDOW_SETTINGS);
    assert!(HOVER_FADE_DURATION_MS > 0);
    assert!(PLACEMENT_SETTLE_DELAY_MS > 0);
    assert!(PET_DRAG_THRESHOLD_PX > 0.0);
    assert!(WINDOW_EDGE_BUFFER > 0);
    assert!(ORB_HITBOX_SIZE > 0);
    assert!(HOVER_POLL_INTERVAL_MS > 0);
    assert!(DRAG_POLL_INTERVAL_MS > 0);
    assert!(STREAM_COALESCE_TIMER_MS > 0);
}
