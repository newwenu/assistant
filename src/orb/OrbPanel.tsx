import React, { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ORB_EXPANDED_WIDTH,
  ORB_EXPANDED_HEIGHT,
  ORB_CHROME_INSET,
  ORB_BLUR_COLLAPSE_DELAY_MS,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { useOrbExpand } from "./OrbExpand";

export const OrbPanel: React.FC = () => {
  const { collapse } = useOrbExpand();

  useEffect(() => {
    let timer: number | null = null;
    const unlisten = listen("orb-blur", () => {
      timer = window.setTimeout(() => {
        if (useOrbStore.getState().isDragging) return;
        collapse();
      }, ORB_BLUR_COLLAPSE_DELAY_MS);
    });
    return () => {
      unlisten.then((fn) => fn());
      if (timer !== null) clearTimeout(timer);
    };
  }, [collapse]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        collapse();
      }
    },
    [collapse],
  );

  return (
    <div
      className="orb-panel"
      style={{
        position: "absolute",
        inset: ORB_CHROME_INSET,
        width: ORB_EXPANDED_WIDTH,
        height: ORB_EXPANDED_HEIGHT,
        background: "var(--ant-color-bg-container, #fff)",
        borderRadius: 12,
        boxShadow: "0 4px 24px rgba(0,0,0,0.15)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        zIndex: 0,
      }}
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      <div
        style={{
          padding: "16px 20px",
          borderBottom: "1px solid var(--ant-color-border-secondary, #f0f0f0)",
          fontWeight: 600,
          fontSize: 14,
        }}
      >
        对话
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--ant-color-text-tertiary, #999)",
          fontSize: 13,
        }}
      >
        I2 阶段实现 AI 对话
      </div>
    </div>
  );
};
