import React, { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import {
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
        const state = useOrbStore.getState();
        if (state.isDragging) return;
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
        const state = useOrbStore.getState();
        if (state.isDragging) return;
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
        top: ORB_CHROME_INSET,
        left: ORB_CHROME_INSET,
        right: ORB_CHROME_INSET,
        bottom: ORB_CHROME_INSET,
        background: "var(--color-bg)",
        borderRadius: "var(--radius-lg)",
        boxShadow: "var(--shadow-panel)",
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
          borderBottom: "1px solid var(--color-border-secondary)",
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
          color: "var(--color-text-tertiary)",
          fontSize: 13,
        }}
      >
        I2 阶段实现 AI 对话
      </div>
      <div
        style={{
          padding: "8px 12px",
          borderTop: "1px solid var(--color-border-secondary)",
          color: "var(--color-text-tertiary)",
          fontSize: 12,
          textAlign: "center",
        }}
      >
        I2 阶段启用输入
      </div>
    </div>
  );
};