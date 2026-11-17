import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ScreenshotSessionDto } from "@/types/protocol.contract";
import { UiEvent } from "@/types/events";
import { useScreenshotStore } from "@/stores/screenshotStore";
import { createLogger } from "@/services/system/logger";
import {
  notifyScreenshotError,
  NO_SESSION,
  screenshotService,
} from "./session";

const log = createLogger("ScreenshotSession");

/**
 * 会话订阅（只在 Orb 窗口调用）：事件是主路径，`get_screenshot_session`
 * 只兜底 reload、错过事件与启动时序。
 */
export function useScreenshotSession(): void {
  useEffect(() => {
    const unPreparing = listen(UiEvent.ScreenshotPreparing, () => {
      /**
       * 菜单/托盘入口由 Rust 发起，前端没有工具栏那次 `beginPreparing()`。这里补上，
       * 否则窗口被撑成全屏时仍渲染小球，绝对定位会把它画到新窗口的原点（左上角）。
       */
      useScreenshotStore.getState().beginPreparing();
    });
    const unSession = listen<ScreenshotSessionDto>(
      UiEvent.ScreenshotSession,
      ({ payload }) => {
        useScreenshotStore.getState().setSession(payload);
      },
    );
    const unError = listen<string>(UiEvent.ScreenshotError, ({ payload }) => {
      useScreenshotStore.getState().endPreparing();
      notifyScreenshotError(payload);
    });

    void screenshotService
      .getSession()
      .then((session) => {
        if (!useScreenshotStore.getState().session) {
          useScreenshotStore.getState().setSession(session);
        }
      })
      .catch((error: unknown) => {
        if (typeof error === "string" && error === NO_SESSION) return;
        log.debug("initial session probe failed", { error });
      });

    return () => {
      void unPreparing.then((fn) => fn());
      void unSession.then((fn) => fn());
      void unError.then((fn) => fn());
    };
  }, []);
}
