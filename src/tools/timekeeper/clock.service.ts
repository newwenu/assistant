import { featureInvoke } from "@/services/featureIpc";
import { timekeeperModule } from "@/features/timekeeper/module";
import type { ClockModeDto, ClockSnapshotDto } from "@/types/protocol.contract";

const invoke = featureInvoke(timekeeperModule);

export const clockService = {
  setMode: (mode: ClockModeDto): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("timekeeper_set_mode", { mode }),

  start: (durationMs?: number): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>(
      "timekeeper_start",
      durationMs === undefined ? {} : { durationMs },
    ),

  pause: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("timekeeper_pause"),

  resume: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("timekeeper_resume"),

  reset: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("timekeeper_reset"),

  snapshot: (): Promise<ClockSnapshotDto> =>
    invoke<ClockSnapshotDto>("timekeeper_snapshot"),
};
