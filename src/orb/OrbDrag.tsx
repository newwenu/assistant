import { useCallback, useRef } from "react";
import {
  getCurrentWindow,
  PhysicalPosition,
  cursorPosition,
} from "@tauri-apps/api/window";
import { orbService } from "../services/orbService";
import {
  PET_DRAG_THRESHOLD_PX,
  DRAG_POLL_INTERVAL_MS,
  ORB_SNAP_DELAY_MS,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { useSettingsStore } from "../stores/settingsStore";

const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);
const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);

export function useOrbDrag() {
  const setDragging = useOrbStore((s) => s.setDragging);
  const startPosRef = useRef<{ x: number; y: number } | null>(null);
  const hasDraggedRef = useRef(false);
  const dragStopRef = useRef<(() => void) | null>(null);

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

        const orbConf = useSettingsStore.getState().orbConf;
        if (orbConf?.click_through) {
          orbService.setOrbClickThrough(false).catch(console.error);
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
            setTimeout(() => {
              orbService.snapOrbToEdge().catch(console.error);
            }, ORB_SNAP_DELAY_MS);
          });
          return;
        }

        void getCurrentWindow()
          .startDragging()
          .finally(() => {
            setDragging(false);
            hasDraggedRef.current = false;
            setTimeout(() => {
              orbService.snapOrbToEdge().catch(console.error);
            }, ORB_SNAP_DELAY_MS);
          });
      }
    },
    [setDragging, startWindowsDrag],
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
          const orbConf = useSettingsStore.getState().orbConf;
          if (orbConf?.click_through) {
            orbService.setOrbClickThrough(true).catch(console.error);
          }
        }
        setTimeout(() => {
          orbService.snapOrbToEdge().catch(console.error);
        }, ORB_SNAP_DELAY_MS);
      }

      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    },
    [setDragging],
  );

  return { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef };
}
