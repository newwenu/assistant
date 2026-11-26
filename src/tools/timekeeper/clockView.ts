import type { ClockSnapshotDto } from "@/types/protocol.contract";

export const PRESET_MINUTES = [15, 25, 45, 60] as const;
export const MIN_MINUTES = 1;
export const MAX_MINUTES = 180;

/** 两套 clock 布局共享的视图数据与动作，状态仍由 index.tsx 持有。 */
export interface ClockViewProps {
  mode: ClockSnapshotDto["mode"];
  status: ClockSnapshotDto["status"];
  timeLabel: string;
  statusLabel: string;
  progress: number | null;
  isFinished: boolean;
  error: string | null;
  busy: boolean;
  minutes: number;
  primaryLabel: string | null;
  onModeChange: (mode: ClockSnapshotDto["mode"]) => void;
  onMinutesChange: (value: number | null) => void;
  onPreset: (minutes: number) => void;
  onStart: () => void;
  onPrimary: () => void;
  onReset: () => void;
}
