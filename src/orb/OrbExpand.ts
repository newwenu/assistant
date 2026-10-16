import { useCallback } from "react";
import { flushSync } from "react-dom";
import { orbService } from "../services/orbService";
import { useOrbStore } from "../stores/orbStore";
import { useSettingsStore } from "../stores/settingsStore";
import {
  ORB_COLLAPSED_SIZE,
  ORB_EXPANDED_WIDTH,
  ORB_EXPANDED_HEIGHT,
  ORB_CHROME_INSET,
  EXPAND_DIRECTION_HYSTERESIS_PX,
} from "../config/constants";
import {
  getWindowLogicalRect,
  physicalToLogical,
  physicalSizeToLogical,
} from "./OrbCoord";

function waitForPaint(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}

const PANEL_TRANSITION_MS = 300;

function waitForPanelTransition(): Promise<void> {
  const panel = document.querySelector<HTMLElement>(".orb-panel");
  if (!panel) return Promise.resolve();
  return new Promise((resolve) => {
    const timeout = setTimeout(resolve, PANEL_TRANSITION_MS + 50);
    const onEnd = (e: TransitionEvent) => {
      if (e.target === panel && e.propertyName === "opacity") {
        clearTimeout(timeout);
        panel.removeEventListener("transitionend", onEnd as EventListener);
        resolve();
      }
    };
    panel.addEventListener("transitionend", onEnd as EventListener);
  });
}

interface UseOrbExpandReturn {
  isExpanded: boolean;
  expand: () => Promise<void>;
  collapse: () => Promise<void>;
}

export function useOrbExpand(): UseOrbExpandReturn {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const storeExpand = useOrbStore((s) => s.expand);
  const storeCollapse = useOrbStore((s) => s.collapse);
  const setExpandDirection = useOrbStore((s) => s.setExpandDirection);
  const setExpandVertical = useOrbStore((s) => s.setExpandVertical);
  const setBallVisible = useOrbStore((s) => s.setBallVisible);

  const expand = useCallback(async () => {
    let direction: "left" | "right" = "right";
    let vertical: "up" | "down" = "down";

    try {
      const rect = await getWindowLogicalRect();
      const ballX = rect.x + ORB_CHROME_INSET;
      const ballY = rect.y + ORB_CHROME_INSET;
      const ballCenterX = ballX + ORB_COLLAPSED_SIZE / 2;
      const ballCenterY = ballY + ORB_COLLAPSED_SIZE / 2;

      const { screen } = await orbService.getScreenSize(
        ballCenterX,
        ballCenterY,
        window.devicePixelRatio || 1,
      );
      const screenPos = physicalToLogical(screen.x, screen.y);
      const screenSize = physicalSizeToLogical(screen.width, screen.height);
      const currentDirection = useOrbStore.getState().expandDirection;
      const currentVertical = useOrbStore.getState().expandVertical;

      const panelW = ORB_EXPANDED_WIDTH;
      const panelH = ORB_EXPANDED_HEIGHT;

      const roomRight = screenPos.x + screenSize.width - ballX;
      const roomLeft = ballX + ORB_COLLAPSED_SIZE - screenPos.x;
      const roomDown = screenPos.y + screenSize.height - ballY;
      const roomUp = ballY + ORB_COLLAPSED_SIZE - screenPos.y;

      const fitsRight = roomRight >= panelW;
      const fitsLeft = roomLeft >= panelW;
      const fitsDown = roomDown >= panelH;
      const fitsUp = roomUp >= panelH;

      if (currentDirection === "right") {
        if (!fitsRight && fitsLeft) {
          direction = "left";
        } else {
          direction = "right";
        }
      } else {
        if (!fitsLeft && fitsRight) {
          direction = "right";
        } else if (
          fitsRight &&
          roomRight >= roomLeft + EXPAND_DIRECTION_HYSTERESIS_PX
        ) {
          direction = "right";
        } else {
          direction = "left";
        }
      }

      if (currentVertical === "down") {
        if (!fitsDown && fitsUp) {
          vertical = "up";
        } else {
          vertical = "down";
        }
      } else {
        if (!fitsUp && fitsDown) {
          vertical = "down";
        } else if (
          fitsDown &&
          roomDown >= roomUp + EXPAND_DIRECTION_HYSTERESIS_PX
        ) {
          vertical = "down";
        } else {
          vertical = "up";
        }
      }

      if (!fitsRight && !fitsLeft) {
        direction = roomRight >= roomLeft ? "right" : "left";
      }
      if (!fitsDown && !fitsUp) {
        vertical = roomDown >= roomUp ? "down" : "up";
      }

      setExpandDirection(direction);
      setExpandVertical(vertical);
    } catch {
      setExpandDirection("right");
      setExpandVertical("down");
    }

    flushSync(() => setBallVisible(false));
    try {
      await waitForPaint();
      await orbService.expandOrbPanel(direction, vertical);
      storeExpand();
      const orbConf = useSettingsStore.getState().orbConf;
      if (orbConf?.clickThrough) {
        await orbService.setOrbClickThrough(false);
      }
    } catch (e) {
      console.error("[OrbExpand] expand invoke failed:", e);
    }
  }, [storeExpand, setExpandDirection, setExpandVertical, setBallVisible]);

  const collapse = useCallback(async () => {
    const direction = useOrbStore.getState().expandDirection;
    const vertical = useOrbStore.getState().expandVertical;
    try {
      storeCollapse();
      await waitForPanelTransition();
      await orbService.collapseOrbPanel(direction, vertical);
      setBallVisible(true);
      const orbConf = useSettingsStore.getState().orbConf;
      if (orbConf?.clickThrough) {
        await orbService.setOrbClickThrough(true);
      }
    } catch (e) {
      console.error("[OrbExpand] collapse invoke failed:", e);
    }
  }, [storeCollapse, setBallVisible]);

  return { isExpanded, expand, collapse };
}

export async function collapseOrbOnInit(): Promise<void> {
  const { expandDirection, expandVertical } = useOrbStore.getState();
  try {
    await orbService.collapseOrbPanel(expandDirection, expandVertical);
    useOrbStore.getState().collapse();
    useOrbStore.getState().setBallVisible(true);
    await useSettingsStore.getState().loadOrbConf();
    const orbConf = useSettingsStore.getState().orbConf;
    if (orbConf?.clickThrough) {
      await orbService.setOrbClickThrough(true);
    }
  } catch (e) {
    console.error("[OrbExpand] collapseOrbOnInit failed:", e);
  }
}
