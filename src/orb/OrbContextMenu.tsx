import React, { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LONG_PRESS_DURATION_MS } from "../config/constants";

const IS_TOUCH_DEVICE =
  "ontouchstart" in window || navigator.maxTouchPoints > 0;

export const OrbContextMenu: React.FC<{
  children: React.ReactNode;
}> = ({ children }) => {
  const longPressTimerRef = useRef<number | null>(null);
  const longPressFiredRef = useRef(false);

  const showMenu = useCallback(async (x: number, y: number) => {
    try {
      await invoke("show_orb_context_menu", { x, y });
    } catch (err) {
      console.error(err);
    }
  }, []);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      void showMenu(e.clientX, e.clientY);
    },
    [showMenu],
  );

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if (!IS_TOUCH_DEVICE) return;
      longPressFiredRef.current = false;
      const touch = e.touches[0];
      longPressTimerRef.current = window.setTimeout(() => {
        longPressFiredRef.current = true;
        void showMenu(touch.clientX, touch.clientY);
      }, LONG_PRESS_DURATION_MS);
    },
    [showMenu],
  );

  const handleTouchMove = useCallback(() => {
    if (longPressTimerRef.current !== null) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (longPressTimerRef.current !== null) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }, []);

  const handleClick = useCallback((e: React.MouseEvent) => {
    if (longPressFiredRef.current) {
      e.preventDefault();
      e.stopPropagation();
      longPressFiredRef.current = false;
    }
  }, []);

  return (
    <div
      onContextMenu={handleContextMenu}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onClick={handleClick}
    >
      {children}
    </div>
  );
};
