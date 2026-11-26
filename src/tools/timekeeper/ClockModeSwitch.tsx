import { Segmented, Tooltip } from "antd";
import { translateKey } from "@/i18n";
import type { ClockSnapshotDto } from "@/types/protocol.contract";
import styles from "./clock.module.css";

interface ClockModeSwitchProps {
  mode: ClockSnapshotDto["mode"];
  disabled: boolean;
  disabledReason?: string;
  compact?: boolean;
  onChange: (mode: ClockSnapshotDto["mode"]) => void;
}

export function ClockModeSwitch({
  mode,
  disabled,
  disabledReason,
  compact = false,
  onChange,
}: ClockModeSwitchProps) {
  return (
    <Tooltip title={disabled ? disabledReason : undefined}>
      <Segmented
        size={compact ? "small" : "large"}
        block
        className={compact ? styles.modeSwitchCompact : styles.modeSwitch}
        value={mode}
        onChange={(value) => {
          if (value === "countdown" || value === "stopwatch") {
            onChange(value);
          }
        }}
        options={[
          {
            value: "countdown",
            label: translateKey("tools.timekeeper.mode_countdown"),
            disabled,
          },
          {
            value: "stopwatch",
            label: translateKey("tools.timekeeper.mode_stopwatch"),
            disabled,
          },
        ]}
      />
    </Tooltip>
  );
}
