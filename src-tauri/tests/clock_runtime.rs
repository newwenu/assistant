use assistant_app::clock::{
    ClockMode, ClockRuntime, ClockStatus, DEFAULT_DURATION_MS, MAX_DURATION_MS, MIN_DURATION_MS,
};

#[test]
fn start_counts_down_by_wall_clock() {
    let rt = ClockRuntime::new();
    let (snapshot, _) = rt.start(Some(60_000), 1_000).unwrap();
    assert_eq!(snapshot.mode, ClockMode::Countdown);
    assert_eq!(snapshot.status, ClockStatus::Running);
    assert_eq!(snapshot.remaining_ms, Some(60_000));
    assert_eq!(snapshot.elapsed_ms, 0);
    assert_eq!(rt.snapshot(2_000).remaining_ms, Some(59_000));
    assert_eq!(rt.snapshot(2_000).elapsed_ms, 1_000);
    assert_eq!(rt.snapshot(2_500).remaining_ms, Some(58_500));
}

#[test]
fn start_without_duration_uses_current_countdown_duration() {
    let rt = ClockRuntime::new();
    let (snapshot, _) = rt.start(None, 1_000).unwrap();
    assert_eq!(snapshot.duration_ms, DEFAULT_DURATION_MS);
    assert_eq!(snapshot.remaining_ms, Some(DEFAULT_DURATION_MS));
}

#[test]
fn pause_freezes_remaining_and_resume_continues() {
    let rt = ClockRuntime::new();
    let (_, _) = rt.start(Some(60_000), 1_000).unwrap();
    let paused = rt.pause(31_000);
    assert_eq!(paused.status, ClockStatus::Paused);
    assert_eq!(paused.remaining_ms, Some(30_000));
    assert_eq!(paused.elapsed_ms, 30_000);
    assert_eq!(rt.snapshot(91_000).remaining_ms, Some(30_000));

    let (resumed, _, was_resumed) = rt.resume(100_000);
    assert!(was_resumed);
    assert_eq!(resumed.status, ClockStatus::Running);
    assert_eq!(resumed.remaining_ms, Some(30_000));
    assert_eq!(rt.snapshot(110_000).remaining_ms, Some(20_000));
}

#[test]
fn invalid_duration_is_rejected() {
    let rt = ClockRuntime::new();
    assert!(rt.start(Some(MIN_DURATION_MS - 1), 1_000).is_err());
    assert!(rt.start(Some(MAX_DURATION_MS + 1), 1_000).is_err());
}

#[test]
fn finish_is_idempotent() {
    let rt = ClockRuntime::new();
    let _ = rt.start(Some(60_000), 1_000).unwrap();
    let finished = rt.mark_finished(61_000).expect("first finish");
    assert_eq!(finished.status, ClockStatus::Finished);
    assert_eq!(finished.remaining_ms, Some(0));
    assert_eq!(finished.elapsed_ms, 60_000);
    assert!(rt.mark_finished(62_000).is_none());
}

#[test]
fn generation_changes_on_state_commands() {
    let rt = ClockRuntime::new();
    let initial = rt.generation();
    let (_, started) = rt.start(Some(60_000), 1_000).unwrap();
    assert!(started > initial);
    let _ = rt.pause(2_000);
    assert!(rt.generation() > started);
    let (_, resumed, was_resumed) = rt.resume(3_000);
    assert!(was_resumed);
    assert_eq!(resumed, rt.generation());
    let _ = rt.reset();
    assert!(rt.generation() > resumed);
}

#[test]
fn reset_keeps_last_duration() {
    let rt = ClockRuntime::new();
    let _ = rt.start(Some(60_000), 1_000).unwrap();
    let reset = rt.reset();
    assert_eq!(reset.status, ClockStatus::Idle);
    assert_eq!(reset.remaining_ms, Some(60_000));
}

#[test]
fn stopwatch_counts_up_by_wall_clock() {
    let rt = ClockRuntime::new();
    let switched = rt.set_mode(ClockMode::Stopwatch, 1_000).unwrap();
    assert_eq!(switched.status, ClockStatus::Idle);
    assert_eq!(switched.elapsed_ms, 0);

    let (snapshot, _) = rt.start(None, 1_000).unwrap();
    assert_eq!(snapshot.mode, ClockMode::Stopwatch);
    assert_eq!(snapshot.status, ClockStatus::Running);
    assert_eq!(snapshot.elapsed_ms, 0);
    assert_eq!(snapshot.remaining_ms, None);
    assert_eq!(rt.snapshot(2_000).elapsed_ms, 1_000);
    assert_eq!(rt.snapshot(11_000).elapsed_ms, 10_000);
}

#[test]
fn stopwatch_pause_freezes_elapsed_and_resume_continues() {
    let rt = ClockRuntime::new();
    rt.set_mode(ClockMode::Stopwatch, 1_000).unwrap();
    let _ = rt.start(None, 1_000).unwrap();

    let paused = rt.pause(31_000);
    assert_eq!(paused.status, ClockStatus::Paused);
    assert_eq!(paused.elapsed_ms, 30_000);
    assert_eq!(paused.remaining_ms, None);
    assert_eq!(rt.snapshot(91_000).elapsed_ms, 30_000);

    let (resumed, _, was_resumed) = rt.resume(100_000);
    assert!(was_resumed);
    assert_eq!(resumed.elapsed_ms, 30_000);
    assert_eq!(rt.snapshot(110_000).elapsed_ms, 40_000);
}

#[test]
fn stopwatch_ignores_duration_argument() {
    let rt = ClockRuntime::new();
    rt.set_mode(ClockMode::Stopwatch, 1_000).unwrap();
    let (snapshot, _) = rt.start(Some(60_000), 1_000).unwrap();
    assert_eq!(snapshot.mode, ClockMode::Stopwatch);
    assert_eq!(snapshot.duration_ms, DEFAULT_DURATION_MS);
    assert_eq!(snapshot.remaining_ms, None);
    assert_eq!(snapshot.elapsed_ms, 0);
}

#[test]
fn stopwatch_never_finishes() {
    let rt = ClockRuntime::new();
    rt.set_mode(ClockMode::Stopwatch, 1_000).unwrap();
    let _ = rt.start(None, 1_000).unwrap();
    assert!(rt.mark_finished(2_000).is_none());
    assert_eq!(rt.snapshot(2_000).status, ClockStatus::Running);
}

#[test]
fn mode_change_is_rejected_while_running() {
    let rt = ClockRuntime::new();
    let _ = rt.start(Some(60_000), 1_000).unwrap();
    assert!(rt.set_mode(ClockMode::Stopwatch, 2_000).is_err());
    assert_eq!(rt.snapshot(2_000).mode, ClockMode::Countdown);
    assert_eq!(rt.snapshot(2_000).status, ClockStatus::Running);
}

#[test]
fn same_mode_is_idempotent_while_running() {
    let rt = ClockRuntime::new();
    let (_, generation) = rt.start(Some(60_000), 1_000).unwrap();
    let snapshot = rt.set_mode(ClockMode::Countdown, 2_000).unwrap();
    assert_eq!(snapshot.status, ClockStatus::Running);
    assert_eq!(snapshot.elapsed_ms, 1_000);
    assert_eq!(rt.generation(), generation);
}

#[test]
fn mode_change_resets_elapsed_and_keeps_duration() {
    let rt = ClockRuntime::new();
    let _ = rt.start(Some(60_000), 1_000).unwrap();
    let _ = rt.pause(31_000);

    let stopwatch = rt.set_mode(ClockMode::Stopwatch, 32_000).unwrap();
    assert_eq!(stopwatch.mode, ClockMode::Stopwatch);
    assert_eq!(stopwatch.status, ClockStatus::Idle);
    assert_eq!(stopwatch.elapsed_ms, 0);
    assert_eq!(stopwatch.remaining_ms, None);
    assert_eq!(stopwatch.duration_ms, 60_000);

    let countdown = rt.set_mode(ClockMode::Countdown, 33_000).unwrap();
    assert_eq!(countdown.mode, ClockMode::Countdown);
    assert_eq!(countdown.status, ClockStatus::Idle);
    assert_eq!(countdown.elapsed_ms, 0);
    assert_eq!(countdown.remaining_ms, Some(60_000));
}
