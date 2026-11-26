use std::sync::Arc;

use tauri::{command, AppHandle, State};

mod ticker;

use assistant_clock::{now_ms, ClockMode, ClockRuntime};
use assistant_protocol::{ClockModeDto, ClockSnapshotDto, Millis};

use ticker::spawn_ticker;

#[command]
pub fn timekeeper_set_mode(
    rt: State<'_, Arc<ClockRuntime>>,
    mode: ClockModeDto,
) -> Result<ClockSnapshotDto, String> {
    Ok(rt.set_mode(mode.into(), now_ms())?.into())
}

#[command]
pub fn timekeeper_start(
    app: AppHandle,
    rt: State<'_, Arc<ClockRuntime>>,
    duration_ms: Option<Millis>,
) -> Result<ClockSnapshotDto, String> {
    let (snapshot, generation) = rt.start(duration_ms, now_ms())?;
    if snapshot.mode == ClockMode::Countdown {
        spawn_ticker(&app, rt.inner().clone(), generation);
    }
    Ok(snapshot.into())
}

#[command]
pub fn timekeeper_pause(rt: State<'_, Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String> {
    Ok(rt.pause(now_ms()).into())
}

#[command]
pub fn timekeeper_resume(
    app: AppHandle,
    rt: State<'_, Arc<ClockRuntime>>,
) -> Result<ClockSnapshotDto, String> {
    let (snapshot, generation, resumed) = rt.resume(now_ms());
    if resumed && snapshot.mode == ClockMode::Countdown {
        spawn_ticker(&app, rt.inner().clone(), generation);
    }
    Ok(snapshot.into())
}

#[command]
pub fn timekeeper_reset(rt: State<'_, Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String> {
    Ok(rt.reset().into())
}

#[command]
pub fn timekeeper_snapshot(rt: State<'_, Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String> {
    Ok(rt.snapshot(now_ms()).into())
}
