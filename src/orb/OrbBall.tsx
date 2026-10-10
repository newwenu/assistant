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
import {
  getMouseLogicalPos,
  getWindowLogicalRect,
  isPointInRect,
} from "./OrbCoord";
import { useOrbDrag } from "./OrbDrag";
import { useOrbExpand } from "./OrbExpand";
import { OrbContextMenu } from "./OrbContextMenu";
import { useOrbConfSync } from "./useOrbConfSync";
import {
  MascotRenderer,
  asRendererId,
  hexToArgb,
  useMascotStore,
} from "../mascot";
import { buildRendererConfig } from "../settings/settingsDispatcher";
import { useSettingsStore } from "../stores/settingsStore";

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);
const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);
const NEEDS_CURSOR_POLL = IS_MACOS || IS_WINDOWS;

export const OrbBall: React.FC = () => {
  useOrbConfSync();
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const isHovering = useOrbStore((s) => s.isHovering);
  const isDragging = useOrbStore((s) => s.isDragging);
  const isBallVisible = useOrbStore((s) => s.isBallVisible);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const setHovering = useOrbStore((s) => s.setHovering);
  const setClickThrough = useOrbStore((s) => s.setClickThrough);
  const mascotFace = useMascotStore((s) => s.face);
  const mascotConf = useSettingsStore((s) => s.mascotConf);
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
      if (debounceTimerRef.current !== null)
        clearTimeout(debounceTimerRef.current);
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

  const ballLeft =
    isExpanded && expandDirection === "left"
      ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
      : ORB_CHROME_INSET;
  const ballTop =
    isExpanded && expandVertical === "up"
      ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
      : ORB_CHROME_INSET;

  const stateClass = isDragging
    ? "orb-dragging"
    : isHovering
      ? "orb-hover"
      : "orb-idle";

  return (
    <OrbContextMenu>
      <div
        className={`orb-hitbox${!isBallVisible ? " orb-ball-hidden" : ""} ${stateClass}`}
        style={{
          position: "absolute",
          top: ballTop,
          left: ballLeft,
          width: ORB_HITBOX_SIZE,
          height: ORB_HITBOX_SIZE,
          zIndex: 1,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onClick={handleClick}
      >
        <MascotRenderer
          renderer={asRendererId(mascotConf?.renderer ?? "rive")}
          face={mascotFace}
          size={ORB_HITBOX_SIZE}
          primaryColor={hexToArgb(mascotConf?.primary_color)}
          secondaryColor={hexToArgb(mascotConf?.secondary_color)}
          rendererConfig={
            mascotConf ? buildRendererConfig(mascotConf) : undefined
          }
        />
      </div>
    </OrbContextMenu>
  );
};
