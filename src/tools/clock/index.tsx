import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { MINI_DENSITY_BREAKPOINT } from "@/config/constants";
import { translateKey } from "@/i18n";
import type { ClockSnapshotDto } from "@/types/protocol.contract";
import { UiEvent } from "@/types/events";
import { clockService } from "./clock.service";
import { ClockLayout } from "./ClockLayout";
import { ClockMiniLayout } from "./ClockMiniLayout";
import { computeClockView, formatClockTime } from "./clockTime";
import { MAX_MINUTES, MIN_MINUTES, type ClockViewProps } from "./clockView";

const DEFAULT_MINUTES = 25;

function statusLabel(status: ClockSnapshotDto["status"]): string {
  switch (status) {
    case "running":
      return translateKey("tools.clock.status_running");
    case "paused":
      return translateKey("tools.clock.status_paused");
    case "finished":
      return translateKey("tools.clock.finished");
    default:
      return translateKey("tools.clock.status_idle");
  }
}

function clampMinutes(value: number): number {
  return Math.min(MAX_MINUTES, Math.max(MIN_MINUTES, value));
}

export default function Clock() {
  const [snapshot, setSnapshot] = useState<ClockSnapshotDto | null>(null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [minutes, setMinutes] = useState<number>(DEFAULT_MINUTES);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [compact, setCompact] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const mode = snapshot?.mode ?? "countdown";
  const status = snapshot?.status ?? "idle";

  const syncMinutesFromSnapshot = useCallback((next: ClockSnapshotDto) => {
    if (next.status === "idle") {
      setMinutes(clampMinutes(Math.round(next.durationMs / 60_000)));
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await clockService.snapshot();
      setSnapshot(next);
      setNowMs(Date.now());
      syncMinutesFromSnapshot(next);
    } catch (e) {
      setError(String(e));
    }
  }, [syncMinutesFromSnapshot]);

  useEffect(() => {
    void refresh();
    const unlisten = listen<ClockSnapshotDto>(
      UiEvent.ClockFinished,
      ({ payload }) => {
        setSnapshot(payload);
        setNowMs(Date.now());
      },
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [refresh]);

  useEffect(() => {
    if (!snapshot || snapshot.status !== "running") return;

    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        setNowMs(Date.now());
      }
    }, 1000);

    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        setNowMs(Date.now());
        void refresh();
      }
    };
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [snapshot, refresh]);

  useLayoutEffect(() => {
    const element = containerRef.current;
    if (!element) return;

    const sync = (width: number) => {
      setCompact(width <= MINI_DENSITY_BREAKPOINT);
    };
    sync(element.clientWidth);

    const observer = new ResizeObserver((observed) => {
      const width = observed[0]?.contentRect.width;
      if (width !== undefined) sync(width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const runCommand = (command: () => Promise<ClockSnapshotDto>) => {
    void (async () => {
      setBusy(true);
      setError(null);
      try {
        const next = await command();
        setSnapshot(next);
        setNowMs(Date.now());
        syncMinutesFromSnapshot(next);
      } catch (e) {
        setError(String(e));
        void refresh();
      } finally {
        setBusy(false);
      }
    })();
  };

  const handleModeChange = (nextMode: ClockSnapshotDto["mode"]) => {
    if (nextMode === mode) return;
    runCommand(() => clockService.setMode(nextMode));
  };

  const handleMinutesChange = (value: number | null) => {
    if (typeof value === "number") {
      setMinutes(clampMinutes(value));
    }
  };

  const handlePreset = (value: number) => {
    setMinutes(clampMinutes(value));
  };

  const handleStart = () => {
    if (mode === "countdown") {
      const clamped = clampMinutes(minutes);
      setMinutes(clamped);
      runCommand(() => clockService.start(clamped * 60_000));
      return;
    }
    runCommand(() => clockService.start());
  };

  const handlePrimary = () => {
    if (status === "running") {
      runCommand(clockService.pause);
    } else if (status === "paused") {
      runCommand(clockService.resume);
    }
  };

  const view = snapshot
    ? computeClockView(snapshot, nowMs)
    : { elapsedMs: 0, remainingMs: minutes * 60_000, progress: 1 };
  const displayMs =
    mode === "countdown"
      ? status === "idle"
        ? minutes * 60_000
        : (view.remainingMs ?? 0)
      : view.elapsedMs;
  const isFinished = mode === "countdown" && status === "finished";
  const primaryLabel =
    status === "running"
      ? translateKey("tools.clock.pause")
      : status === "paused"
        ? translateKey("tools.clock.resume")
        : null;

  const viewProps: ClockViewProps = {
    mode,
    status,
    timeLabel: formatClockTime(displayMs, mode),
    statusLabel: statusLabel(status),
    progress:
      mode === "countdown" ? (status === "idle" ? 1 : view.progress) : null,
    isFinished,
    error,
    busy,
    minutes,
    primaryLabel,
    onModeChange: handleModeChange,
    onMinutesChange: handleMinutesChange,
    onPreset: handlePreset,
    onStart: handleStart,
    onPrimary: handlePrimary,
    onReset: () => runCommand(clockService.reset),
  };

  return (
    <div ref={containerRef}>
      {compact ? (
        <ClockMiniLayout {...viewProps} />
      ) : (
        <ClockLayout {...viewProps} />
      )}
    </div>
  );
}
