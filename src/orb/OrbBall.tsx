import React, { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ORB_HITBOX_SIZE,
  ORB_CHROME_INSET,
  HOVER_POLL_INTERVAL_MS,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { useOrbDrag } from "./OrbDrag";
import { useOrbExpand } from "./OrbExpand";
import { OrbContextMenu } from "./OrbContextMenu";

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);

export const OrbBall: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const isHovering = useOrbStore((s) => s.isHovering);
  const isDragging = useOrbStore((s) => s.isDragging);
  const setHovering = useOrbStore((s) => s.setHovering);
  const { expand } = useOrbExpand();
  const { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef } =
    useOrbDrag();
  const hoverTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!IS_MACOS) return;

    hoverTimerRef.current = window.setInterval(async () => {
      try {
        const pos = await invoke<{ clientX: number; clientY: number }>(
          "get_mouse_position",
        );
        const win = getCurrentWindow();
        const winPos = await win.outerPosition();
        const winSize = await win.innerSize();
        const inside =
          pos.clientX >= winPos.x &&
          pos.clientX < winPos.x + winSize.width &&
          pos.clientY >= winPos.y &&
          pos.clientY < winPos.y + winSize.height;
        setHovering(inside);
        const state = useOrbStore.getState();
        if (inside) {
          await invoke("set_orb_click_through", { enabled: false });
        } else if (!state.isDragging && !state.isExpanded) {
          await invoke("set_orb_click_through", { enabled: true });
        }
      } catch {
        // ignore
      }
    }, HOVER_POLL_INTERVAL_MS);
    return () => {
      if (hoverTimerRef.current !== null) clearInterval(hoverTimerRef.current);
    };
  }, [setHovering]);

  const handlePointerEnter = useCallback(() => {
    if (IS_MACOS) return;
    setHovering(true);
  }, [setHovering]);

  const handlePointerLeave = useCallback(() => {
    if (IS_MACOS) return;
    setHovering(false);
  }, [setHovering]);

  const handleClick = useCallback(() => {
    if (hasDraggedRef.current) return;
    if (!isExpanded) {
      expand();
    }
  }, [isExpanded, expand, hasDraggedRef]);

  const opacity = isDragging ? 0.7 : isHovering ? 1.0 : 0.85;

  return (
    <OrbContextMenu>
      <div
        className="orb-hitbox"
        style={{
          position: "absolute",
          top: ORB_CHROME_INSET,
          left: ORB_CHROME_INSET,
          width: ORB_HITBOX_SIZE,
          height: ORB_HITBOX_SIZE,
          borderRadius: "50%",
          background: "radial-gradient(circle at 30% 30%, #6ea8fe, #3b82f6)",
          boxShadow: isExpanded
            ? "0 0 0 1px var(--color-border, #e8e8e8)"
            : "0 2px 8px rgba(0,0,0,0.3)",
          cursor: "grab",
          userSelect: "none",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          opacity,
          transition: "opacity 150ms ease-in-out, box-shadow 300ms ease-in-out",
          zIndex: 1,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onClick={handleClick}
      >
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="4" fill="white" opacity="0.9" />
        </svg>
      </div>
    </OrbContextMenu>
  );
};
