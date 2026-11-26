import { Alert, Button, InputNumber, Space, Typography } from "antd";
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

export function ClockLayout({
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
  const showDuration = mode === "countdown";
  const durationEditable = status === "idle" || status === "finished";
  const showStopwatchStart = mode === "stopwatch" && status === "idle";

  return (
    <div className={styles.clock}>
      <ClockModeSwitch
        mode={mode}
        disabled={status === "running" || busy}
        disabledReason={
          status === "running"
            ? translateKey("tools.timekeeper.mode_switch_running")
            : undefined
        }
        onChange={onModeChange}
      />

      <ClockRing timeLabel={timeLabel} progress={progress} />

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
        <>
          <Space wrap size={8} className={styles.presets}>
            {PRESET_MINUTES.map((preset) => (
              <Button
                key={preset}
                disabled={busy || !durationEditable}
                aria-pressed={minutes === preset}
                className={minutes === preset ? styles.presetActive : undefined}
                onClick={() => onPreset(preset)}
              >
                {preset} {translateKey("tools.timekeeper.minutes")}
              </Button>
            ))}
          </Space>

          <div className={styles.customRow}>
            <InputNumber
              min={MIN_MINUTES}
              max={MAX_MINUTES}
              value={minutes}
              disabled={busy || !durationEditable}
              aria-label={translateKey("tools.timekeeper.custom_minutes")}
              onChange={onMinutesChange}
              style={{ width: 110 }}
            />
            <Typography.Text type="secondary" className={styles.unit}>
              {translateKey("tools.timekeeper.minutes")}
            </Typography.Text>
            {durationEditable && (
              <Button type="primary" disabled={busy} onClick={onStart}>
                {translateKey("tools.timekeeper.start")}
              </Button>
            )}
          </div>
        </>
      )}

      <Space wrap size={8} className={styles.controls}>
        {primaryLabel ? (
          <Button type="primary" disabled={busy} onClick={onPrimary}>
            {primaryLabel}
          </Button>
        ) : showStopwatchStart ? (
          <Button type="primary" disabled={busy} onClick={onStart}>
            {translateKey("tools.timekeeper.start")}
          </Button>
        ) : null}
        <Button disabled={busy || status === "idle"} onClick={onReset}>
          {translateKey("tools.timekeeper.reset")}
        </Button>
      </Space>

      {mode === "stopwatch" && status === "idle" && (
        <Typography.Text type="secondary" className={styles.hint}>
          {translateKey("tools.timekeeper.stopwatch_hint")}
        </Typography.Text>
      )}

      {mode === "countdown" && isFinished && (
        <Typography.Text type="secondary" className={styles.hint}>
          {translateKey("tools.timekeeper.finished_hint")}
        </Typography.Text>
      )}
    </div>
  );
}
