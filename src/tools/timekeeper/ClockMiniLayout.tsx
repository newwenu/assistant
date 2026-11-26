import { Alert, Button, InputNumber, Segmented, Space, Typography } from "antd";
import { translateKey } from "@/i18n";
import { ClockModeSwitch } from "./ClockModeSwitch";
import { ClockRing } from "./ClockRing";
import {
  MAX_MINUTES,
  MIN_MINUTES,
  PRESET_MINUTES,
  type ClockViewProps,
} from "./clockView";
import styles from "./clock.module.css";

export function ClockMiniLayout({
  mode,
  status,
  timeLabel,
  statusLabel,
  progress,
  isFinished,
  error,
  busy,
  minutes,
  primaryLabel,
  onModeChange,
  onMinutesChange,
  onPreset,
  onStart,
  onPrimary,
  onReset,
}: ClockViewProps) {
  const showDuration =
    mode === "countdown" && (status === "idle" || status === "finished");
  const showStopwatchStart = mode === "stopwatch" && status === "idle";
  const selectedPreset = PRESET_MINUTES.find((preset) => preset === minutes);
  const showControls =
    primaryLabel !== null ||
    showStopwatchStart ||
    (mode === "countdown" && status === "finished");

  return (
    <div className={`${styles.clock} ${styles.clockCompact}`}>
      <ClockModeSwitch
        mode={mode}
        disabled={status === "running" || busy}
        disabledReason={
          status === "running"
            ? translateKey("tools.timekeeper.mode_switch_running")
            : undefined
        }
        compact
        onChange={onModeChange}
      />

      <ClockRing compact timeLabel={timeLabel} progress={progress} />

      <Typography.Text
        type="secondary"
        className={styles.status}
        style={{ color: isFinished ? "var(--color-success)" : undefined }}
      >
        {statusLabel}
      </Typography.Text>

      {error && (
        <Alert
          type="error"
          showIcon
          className={styles.alert}
          title={translateKey("tools.timekeeper.error")}
          description={error}
        />
      )}

      {showDuration && (
        <div className={styles.durationPicker}>
          <Segmented
            size="small"
            block
            className={styles.presetSegmented}
            disabled={busy}
            value={selectedPreset}
            onChange={(value) => {
              if (typeof value === "number") {
                onPreset(value);
              }
            }}
            options={PRESET_MINUTES.map((preset) => ({
              value: preset,
              label: String(preset),
            }))}
          />

          <div className={styles.customRow}>
            <InputNumber
              size="small"
              min={MIN_MINUTES}
              max={MAX_MINUTES}
              value={minutes}
              aria-label={translateKey("tools.timekeeper.custom_minutes")}
              onChange={onMinutesChange}
              style={{ width: 72 }}
            />
            <Typography.Text type="secondary" className={styles.unit}>
              {translateKey("tools.timekeeper.minutes")}
            </Typography.Text>
            <Button
              type="primary"
              size="small"
              disabled={busy}
              onClick={onStart}
            >
              {translateKey("tools.timekeeper.start")}
            </Button>
          </div>
        </div>
      )}

      {showControls && (
        <Space wrap size={6} className={styles.controls}>
          {primaryLabel ? (
            <Button type="primary" disabled={busy} onClick={onPrimary}>
              {primaryLabel}
            </Button>
          ) : showStopwatchStart ? (
            <Button type="primary" disabled={busy} onClick={onStart}>
              {translateKey("tools.timekeeper.start")}
            </Button>
          ) : null}
          <Button
            size="small"
            disabled={busy || status === "idle"}
            onClick={onReset}
          >
            {translateKey("tools.timekeeper.reset")}
          </Button>
        </Space>
      )}
    </div>
  );
}
