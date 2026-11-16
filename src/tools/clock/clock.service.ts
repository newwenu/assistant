import { tauriInvoke as invoke } from "@/services/system/tauriInvoke";
import type { ClockModeDto, ClockSnapshotDto } from "@/types/protocol.contract";

export const clockService = {
  setMode: (mode: ClockModeDto): Promise<ClockSnapshotDto> =>
    invoke("clock_set_mode", { mode }),

  start: (durationMs?: number): Promise<ClockSnapshotDto> =>
    invoke("clock_start", durationMs === undefined ? {} : { durationMs }),

  pause: (): Promise<ClockSnapshotDto> => invoke("clock_pause"),

  resume: (): Promise<ClockSnapshotDto> => invoke("clock_resume"),

  reset: (): Promise<ClockSnapshotDto> => invoke("clock_reset"),

  snapshot: (): Promise<ClockSnapshotDto> => invoke("clock_snapshot"),
};
