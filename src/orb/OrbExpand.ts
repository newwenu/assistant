import { useCallback } from "react";
import { flushSync } from "react-dom";
import { orbService } from "../services/desktop/orbService";
import { useOrbStore } from "../stores/orbStore";
import { useScreenshotStore } from "../stores/screenshotStore";
import { useSettingsStore } from "../stores/settingsStore";
import { getConfValue } from "@/settings/core/runtime/useConfGroup";
import { createLogger } from "@/services/system/logger";
import { isOrbBusy } from "./orbBusy";
import { closeOrbOverlays } from "./orbOverlays";
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

const log = createLogger("OrbExpand");

function waitForPaint(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}

const PANEL_TRANSITION_MS = 300;

function waitForPanelTransition(): Promise<void> {
  if (typeof document === "undefined") return Promise.resolve();
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

let collapsingPromise: Promise<void> | null = null;
let blurCollapseSuppressedUntil = 0;

/** 面板内的主动打开动作会抢走焦点，用时间窗屏蔽失焦自动收起，避免与显式收起重复。 */
export function suppressOrbBlurCollapse(ms = 600): void {
  blurCollapseSuppressedUntil = Date.now() + ms;
}

export function isOrbBlurCollapseSuppressed(): boolean {
  if (Date.now() < blurCollapseSuppressedUntil) return true;
  /** 截图会话期间窗口被撑成全屏且必然失焦，此时折叠会把几何写坏（不变式 16）。 */
  const shot = useScreenshotStore.getState();
  return shot.session !== null || shot.preparing || shot.finishing;
}

/** 收起唯一入口：并发调用合并为同一次，已收起时直接返回。 */
export async function collapseOrbPanel(): Promise<void> {
  closeOrbOverlays();
  if (collapsingPromise) return collapsingPromise;
  if (!useOrbStore.getState().isExpanded) return;

  const run = (async () => {
    const { expandDirection, expandVertical } = useOrbStore.getState();
    try {
      useOrbStore.getState().collapse();
      await waitForPanelTransition();
      await orbService.collapseOrbPanel(expandDirection, expandVertical);
      useOrbStore.getState().setBallVisible(true);
      const orbConf = getConfValue("orb");
      if (orbConf?.clickThrough) {
        await orbService.setOrbClickThrough(true);
      }
    } catch (e) {
      log.error("collapse invoke failed", e);
    } finally {
      collapsingPromise = null;
    }
  })();

  collapsingPromise = run;
  return run;
}

/**
 * 自动收起入口（I47 §4.3）：任务进行中时拦截，显式收起不走这里。
 *
 * 提前返回不改任何 store 状态、不调用任何 `orbService` 方法，
 * 因此不会触发「收起后恢复穿透」。
 */
export async function autoCollapseOrb(): Promise<void> {
  if (isOrbBusy()) return;
  await collapseOrbPanel();
}

interface UseOrbExpandReturn {
  isExpanded: boolean;
  expand: () => Promise<void>;
  collapse: () => Promise<void>;
  autoCollapse: () => Promise<void>;
}

export function useOrbExpand(): UseOrbExpandReturn {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const storeExpand = useOrbStore((s) => s.expand);
  const setExpandDirection = useOrbStore((s) => s.setExpandDirection);
  const setExpandVertical = useOrbStore((s) => s.setExpandVertical);
  const setBallVisible = useOrbStore((s) => s.setBallVisible);

  const expand = useCallback(async () => {
    if (collapsingPromise) await collapsingPromise;
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
    } catch (e) {
      log.warn("Screen size detection failed, using defaults", e);
      setExpandDirection("right");
      setExpandVertical("down");
    }

    flushSync(() => setBallVisible(false));
    try {
      await waitForPaint();
      await orbService.expandOrbPanel(direction, vertical);
      storeExpand();
      const orbConf = getConfValue("orb");
      if (orbConf?.clickThrough) {
        await orbService.setOrbClickThrough(false);
      }
    } catch (e) {
      log.error("expand invoke failed", e);
    }
  }, [storeExpand, setExpandDirection, setExpandVertical, setBallVisible]);

  const collapse = useCallback(() => collapseOrbPanel(), []);
  const autoCollapse = useCallback(() => autoCollapseOrb(), []);

  return { isExpanded, expand, collapse, autoCollapse };
}

export async function collapseOrbOnInit(): Promise<void> {
  const { expandDirection, expandVertical } = useOrbStore.getState();
  try {
    await orbService.collapseOrbPanel(expandDirection, expandVertical);
    useOrbStore.getState().collapse();
    useOrbStore.getState().setBallVisible(true);
    await useSettingsStore.getState().loadConf("orb");
    const orbConf = getConfValue("orb");
    if (orbConf?.clickThrough) {
      await orbService.setOrbClickThrough(true);
    }
  } catch (e) {
    log.error("collapseOrbOnInit failed", e);
  }
}
