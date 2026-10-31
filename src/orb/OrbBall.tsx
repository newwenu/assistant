import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  ORB_HITBOX_SIZE,
  ORB_COLLAPSED_SIZE,
  ORB_CHROME_INSET,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { useOrbDrag } from "./OrbDrag";
import { useOrbExpand, collapseOrbOnInit } from "./OrbExpand";
import { OrbContextMenu } from "./OrbContextMenu";
import { useOrbConfSync } from "./useOrbConfSync";
import {
  MascotRenderer,
  asRendererId,
  hexToArgb,
  useMascotStore,
} from "../mascot";
import { buildRendererConfig } from "../mascot/core/mascotConfig";
import { useConfGroup } from "@/settings/core/useConfGroup";

export const OrbBall: React.FC = () => {
  const { ready: confReady } = useOrbConfSync();
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const isHovering = useOrbStore((s) => s.isHovering);
  const isDragging = useOrbStore((s) => s.isDragging);
  const isBallVisible = useOrbStore((s) => s.isBallVisible);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const setHovering = useOrbStore((s) => s.setHovering);
  const mascotFace = useMascotStore((s) => s.face);
  const mascotConf = useConfGroup("mascot");
  const { expand } = useOrbExpand();
  const { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef } =
    useOrbDrag();

  const [settled, setSettled] = useState(false);
  const [settledIn, setSettledIn] = useState(false);
  const initGuardRef = useRef(false);
  const confReadyRef = useRef(confReady);
  confReadyRef.current = confReady;
  useEffect(() => {
    if (initGuardRef.current) return;
    initGuardRef.current = true;
    void (async () => {
      await confReadyRef.current;
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          void collapseOrbOnInit().then(() => {
            requestAnimationFrame(() => {
              setSettled(true);
              setSettledIn(true);
              requestAnimationFrame(() => setSettledIn(false));
            });
          });
        });
      });
    })();
  }, []);

  const handlePointerEnter = useCallback(() => {
    if (!settled) return;
    setHovering(true);
  }, [setHovering, settled]);

  const handlePointerLeave = useCallback(() => {
    setHovering(false);
  }, [setHovering]);

  const handleClick = useCallback(() => {
    if (hasDraggedRef.current) return;
    if (!isExpanded) {
      void expand();
    }
  }, [isExpanded, expand, hasDraggedRef]);

  const ballLeft =
    isExpanded && expandDirection === "left"
      ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
      : 0;
  const ballTop =
    isExpanded && expandVertical === "up"
      ? `calc(100% - ${ORB_CHROME_INSET + ORB_COLLAPSED_SIZE}px)`
      : 0;

  const stateClass = isDragging
    ? "orb-dragging"
    : isHovering
      ? "orb-hover"
      : "orb-idle";

  return (
    <OrbContextMenu>
      <div
        className={`orb-hitbox${!isBallVisible ? " orb-ball-hidden" : ""}${!settled ? " orb-settling" : ""}${settledIn ? " orb-settled-in" : ""} ${stateClass}`}
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
          size={ORB_COLLAPSED_SIZE}
          primaryColor={hexToArgb(mascotConf?.primaryColor)}
          secondaryColor={hexToArgb(mascotConf?.secondaryColor)}
          rendererConfig={
            mascotConf ? buildRendererConfig(mascotConf) : undefined
          }
        />
      </div>
    </OrbContextMenu>
  );
};
