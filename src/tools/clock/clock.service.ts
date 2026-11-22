import { featureInvoke } from "@/services/featureIpc";
import { clockModule } from "@/features/clock/module";
import type { ClockModeDto, ClockSnapshotDto } from "@/types/protocol.contract";

const invoke = featureInvoke(clockModule);

export const clockService = {
  setMode: (mode: ClockModeDto): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("clock_set_mode", { mode }),

  start: (durationMs?: number): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>(
      "clock_start",
      durationMs === undefined ? {} : { durationMs },
    ),

  pause: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("clock_pause"),

  resume: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("clock_resume"),

  reset: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("clock_reset"),

  snapshot: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("clock_snapshot"),
};
