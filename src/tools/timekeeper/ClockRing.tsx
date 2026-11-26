import styles from "./clock.module.css";

const RING_RADIUS = 70;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

interface ClockRingProps {
  timeLabel: string;
  progress: number | null;
  compact?: boolean;
}

export function ClockRing({
  timeLabel,
  progress,
  compact = false,
}: ClockRingProps) {
  const dashOffset =
    progress === null ? 0 : RING_CIRCUMFERENCE * (1 - progress);

  return (
    <div className={compact ? styles.ringCompact : styles.ring}>
      <svg
        className={styles.ringSvg}
        viewBox="0 0 160 160"
        role="timer"
        aria-label={timeLabel}
      >
        <circle
          className={styles.ringTrack}
          cx="80"
          cy="80"
          r={RING_RADIUS}
          fill="none"
        />
        {progress !== null && (
          <circle
            className={styles.ringProgress}
            cx="80"
            cy="80"
            r={RING_RADIUS}
            fill="none"
            strokeDasharray={RING_CIRCUMFERENCE}
            strokeDashoffset={dashOffset}
          />
        )}
      </svg>
      <div className={compact ? styles.timeCompact : styles.time}>
        {timeLabel}
      </div>
    </div>
  );
}
