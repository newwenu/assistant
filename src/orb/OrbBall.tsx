import React, { useCallback, useEffect, useState } from "react";
import {
  ORB_HITBOX_SIZE,
  ORB_COLLAPSED_SIZE,
  ORB_CHROME_INSET,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
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
import { buildRendererConfig } from "../settings/core/settingsDispatcher";
import { useSettingsStore } from "../stores/settingsStore";

export const OrbBall: React.FC = () => {
  useOrbConfSync();
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const isHovering = useOrbStore((s) => s.isHovering);
  const isDragging = useOrbStore((s) => s.isDragging);
  const isBallVisible = useOrbStore((s) => s.isBallVisible);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const setHovering = useOrbStore((s) => s.setHovering);
  const mascotFace = useMascotStore((s) => s.face);
  const mascotConf = useSettingsStore((s) => s.mascotConf);
  const { expand } = useOrbExpand();
  const { onPointerDown, onPointerMove, onPointerUp, hasDraggedRef } =
    useOrbDrag();

  const [settled, setSettled] = useState(false);
  useEffect(() => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        setSettled(true);
      });
    });
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
        className={`orb-hitbox${!isBallVisible ? " orb-ball-hidden" : ""}${!settled ? " orb-settling" : ""} ${stateClass}`}
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
