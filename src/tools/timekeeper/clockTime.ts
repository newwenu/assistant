import type { ClockSnapshotDto } from "@/types/protocol.contract";

export interface ClockView {
  elapsedMs: number;
  remainingMs: number | null;
  progress: number | null;
}

/**
 * 由快照做本地续算。运行时以 elapsedBeforeMs + startedAtMs 为准，
 * 不复用快照里的 remainingMs，避免二次扣除。
 */
export function computeClockView(
  snapshot: ClockSnapshotDto,
  nowMs: number,
): ClockView {
  const runningDelta =
    snapshot.status === "running" && snapshot.startedAtMs !== null
      ? Math.max(0, nowMs - snapshot.startedAtMs)
      : 0;
  const elapsed = snapshot.elapsedBeforeMs + runningDelta;

  if (snapshot.mode === "countdown") {
    const remaining = Math.max(
      0,
      Math.min(snapshot.durationMs, snapshot.durationMs - elapsed),
    );
    return {
      elapsedMs: snapshot.durationMs - remaining,
      remainingMs: remaining,
      progress: snapshot.durationMs > 0 ? remaining / snapshot.durationMs : 0,
    };
  }

  return { elapsedMs: elapsed, remainingMs: null, progress: null };
}

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

/** 倒计时剩余向上取整，普通计时已用向下取整；≥ 1 小时使用 h:mm:ss。 */
export function formatClockTime(
  ms: number,
  mode: ClockSnapshotDto["mode"],
): string {
  const totalSeconds = Math.max(
    0,
    mode === "countdown" ? Math.ceil(ms / 1000) : Math.floor(ms / 1000),
  );
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${pad2(minutes)}:${pad2(seconds)}`;
  }
  return `${pad2(minutes)}:${pad2(seconds)}`;
}
