import React, { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ORB_HITBOX_SIZE,
  ORB_COLLAPSED_SIZE,
  ORB_CHROME_INSET,
  HOVER_POLL_INTERVAL_MS,
  CLICK_THROUGH_DEBOUNCE_MS,
  HAND_JITTER_PX,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { getMouseLogicalPos, getWindowLogicalRect, isPointInRect } from "./OrbCoord";
import { useOrbDrag } from "./OrbDrag";
import { useOrbExpand } from "./OrbExpand";
import { OrbContextMenu } from "./OrbContextMenu";

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);
const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);
const NEEDS_CURSOR_POLL = IS_MACOS || IS_WINDOWS;

export const OrbBall: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const isHovering = useOrbStore((s) => s.isHovering);
  const isDragging = useOrbStore((s) => s.isDragging);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const setHovering = useOrbStore((s) => s.setHovering);
  const setClickThrough = useOrbStore((s) => s.setClickThrough);
  const { expand } = useOrbExpand();
  const { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef } =
    useOrbDrag();
  const hoverTimerRef = useRef<number | null>(null);
  const debounceTimerRef = useRef<number | null>(null);
  const lastInsidePos = useRef<{ x: number; y: number } | null>(null);

  const hasOpenOverlay = useCallback(() => {
    return !!document.querySelector(
      ".ant-dropdown:not(.ant-dropdown-hidden), .ant-popover:not(.ant-popover-hidden), .ant-modal-root",
    );
  }, []);

  const applyClickThrough = useCallback(
    (enabled: boolean, debounce: boolean = false) => {
      if (debounceTimerRef.current !== null) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }

      const current = useOrbStore.getState().clickThrough;
      if (enabled === current) return;

      if (enabled && debounce) {
        debounceTimerRef.current = window.setTimeout(() => {
          invoke("set_orb_click_through", { enabled: true }).catch(
            console.error,
          );
          setClickThrough(true);
          debounceTimerRef.current = null;
        }, CLICK_THROUGH_DEBOUNCE_MS);
      } else {
        invoke("set_orb_click_through", { enabled }).catch(console.error);
        setClickThrough(enabled);
      }
    },
    [setClickThrough],
  );

  useEffect(() => {
    if (!NEEDS_CURSOR_POLL) return;

    hoverTimerRef.current = window.setInterval(async () => {
      try {
        const [mouse, winRect] = await Promise.all([
          getMouseLogicalPos(),
          getWindowLogicalRect(),
        ]);
        const state = useOrbStore.getState();
        const { x: mx, y: my } = mouse;

        const insideWindow = isPointInRect(mx, my, winRect);

        if (state.isExpanded) {
          setHovering(insideWindow);
          if (insideWindow) {
            applyClickThrough(false);
          } else {
            applyClickThrough(true, true);
          }
          return;
        }

        if (hasOpenOverlay()) {
          applyClickThrough(false);
          return;
        }

        const hitboxRect = {
          x: winRect.x + ORB_CHROME_INSET,
          y: winRect.y + ORB_CHROME_INSET,
          width: ORB_HITBOX_SIZE,
          height: ORB_HITBOX_SIZE,
        };
        const insideHitbox = isPointInRect(mx, my, hitboxRect);

        setHovering(insideHitbox);

        if (insideHitbox) {
          lastInsidePos.current = { x: mx, y: my };
          applyClickThrough(false);
        } else if (!state.isDragging) {
          if (lastInsidePos.current) {
            const dist = Math.hypot(
              mx - lastInsidePos.current.x,
              my - lastInsidePos.current.y,
            );
            if (dist <= HAND_JITTER_PX) {
              return;
            }
            lastInsidePos.current = null;
          }
          applyClickThrough(true, true);
        }
      } catch {
        // ignore
      }
    }, HOVER_POLL_INTERVAL_MS);
    return () => {
      if (hoverTimerRef.current !== null) clearInterval(hoverTimerRef.current);
      if (debounceTimerRef.current !== null) clearTimeout(debounceTimerRef.current);
    };
  }, [setHovering, applyClickThrough]);

  const handlePointerEnter = useCallback(() => {
    if (NEEDS_CURSOR_POLL) return;
    setHovering(true);
  }, [setHovering]);

  const handlePointerLeave = useCallback(() => {
    if (NEEDS_CURSOR_POLL) return;
    setHovering(false);
  }, [setHovering]);

  const handleClick = useCallback(() => {
    if (hasDraggedRef.current) return;
    if (!isExpanded) {
      expand();
    }
  }, [isExpanded, expand, hasDraggedRef]);

  const opacity = isDragging ? 0.7 : isHovering ? 1.0 : 0.85;

  const ballLeft = isExpanded && expandDirection === "left"
    ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
    : ORB_CHROME_INSET;
  const ballTop = isExpanded && expandVertical === "up"
    ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
    : ORB_CHROME_INSET;

  const ballStyle: React.CSSProperties = {
    position: "absolute",
    top: ballTop,
    left: ballLeft,
  };

  return (
    <OrbContextMenu>
      <div
        className="orb-hitbox"
        style={{
          ...ballStyle,
          width: ORB_HITBOX_SIZE,
          height: ORB_HITBOX_SIZE,
          borderRadius: "50%",
          background:
            "radial-gradient(circle at 30% 30%, var(--color-orb-gradient-start), var(--color-orb-gradient-end))",
          boxShadow: "none",
          filter: isExpanded
            ? "none"
            : "var(--filter-orb)",
          cursor: "grab",
          userSelect: "none",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          opacity,
          transition: "opacity 150ms ease-in-out, filter 300ms ease-in-out",
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