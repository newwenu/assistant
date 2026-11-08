import { useCallback, useRef } from "react";
import {
  getCurrentWindow,
  PhysicalPosition,
  cursorPosition,
} from "@tauri-apps/api/window";
import { orbService } from "../services/orbService";
import { createLogger } from "@/services/logger";
import {
  PET_DRAG_THRESHOLD_PX,
  DRAG_POLL_INTERVAL_MS,
  ORB_SNAP_DELAY_MS,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { getConfValue } from "@/settings/core/useConfGroup";

const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);
const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);
const log = createLogger("OrbDrag");

export function useOrbDrag() {
  const setDragging = useOrbStore((s) => s.setDragging);
  const startPosRef = useRef<{ x: number; y: number } | null>(null);
  const hasDraggedRef = useRef(false);
  const dragStopRef = useRef<(() => void) | null>(null);
  const snapTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelSnap = useCallback(() => {
    if (snapTimeoutRef.current !== null) {
      clearTimeout(snapTimeoutRef.current);
      snapTimeoutRef.current = null;
    }
  }, []);

  const scheduleSnap = useCallback(() => {
    cancelSnap();
    snapTimeoutRef.current = setTimeout(() => {
      snapTimeoutRef.current = null;
      orbService
        .snapOrbToEdge()
        .catch((e) => log.warn("OrbDrag operation failed", e));
    }, ORB_SNAP_DELAY_MS);
  }, [cancelSnap]);

  const startWindowsDrag = useCallback(() => {
    const appWindow = getCurrentWindow();
    let stopped = false;
    const stop = () => {
      stopped = true;
    };
    void Promise.all([appWindow.outerPosition(), cursorPosition()]).then(
      ([origin, grab]) => {
        let prevX = grab.x;
        let prevY = grab.y;
        const tick = async () => {
          if (stopped) return;
          try {
            const pointer = await cursorPosition();
            if (stopped) return;
            if (pointer.x !== prevX || pointer.y !== prevY) {
              prevX = pointer.x;
              prevY = pointer.y;
              await appWindow.setPosition(
                new PhysicalPosition(
                  origin.x + pointer.x - grab.x,
                  origin.y + pointer.y - grab.y,
                ),
              );
            }
            if (!stopped) setTimeout(() => void tick(), DRAG_POLL_INTERVAL_MS);
          } catch {
            stop();
          }
        };
        void tick();
      },
    );
    return stop;
  }, []);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    startPosRef.current = { x: e.clientX, y: e.clientY };
    hasDraggedRef.current = false;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!startPosRef.current) return;
      const dx = e.clientX - startPosRef.current.x;
      const dy = e.clientY - startPosRef.current.y;
      if (
        !hasDraggedRef.current &&
        Math.sqrt(dx * dx + dy * dy) < PET_DRAG_THRESHOLD_PX
      ) {
        return;
      }
      if (!hasDraggedRef.current) {
        hasDraggedRef.current = true;
        setDragging(true);
        cancelSnap();

        const orbConf = getConfValue("orb");
        if (orbConf?.clickThrough) {
          orbService
            .setOrbClickThrough(false)
            .catch((e) => log.warn("OrbDrag operation failed", e));
        }

        if (IS_WINDOWS) {
          dragStopRef.current = startWindowsDrag();
          return;
        }

        if (IS_MACOS) {
          const appWindow = getCurrentWindow();
          void appWindow.startDragging().finally(() => {
            setDragging(false);
            hasDraggedRef.current = false;
            scheduleSnap();
          });
          return;
        }

        void getCurrentWindow()
          .startDragging()
          .finally(() => {
            setDragging(false);
            hasDraggedRef.current = false;
            scheduleSnap();
          });
      }
    },
    [setDragging, startWindowsDrag, scheduleSnap, cancelSnap],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (!startPosRef.current) return;
      startPosRef.current = null;

      if (dragStopRef.current) {
        dragStopRef.current();
        dragStopRef.current = null;
      }

      if (hasDraggedRef.current) {
        setDragging(false);
        const state = useOrbStore.getState();
        if (!state.isExpanded) {
          const orbConf = getConfValue("orb");
          if (orbConf?.clickThrough) {
            orbService
              .setOrbClickThrough(true)
              .catch((e) => log.warn("OrbDrag operation failed", e));
          }
        }
        scheduleSnap();
      }

      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    },
    [setDragging, scheduleSnap],
  );

  return { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef };
}
