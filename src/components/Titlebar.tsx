import { useMemo } from "react";
import { ThemeToggle } from "./ThemeToggle";
import { WindowControls } from "./WindowControls";
import {
  TITLEBAR_HEIGHT,
  MACOS_TRAFFIC_LIGHT_OFFSET,
  TITLEBAR_PADDING_LEFT,
} from "@/config/constants";

interface TitlebarProps {
  title?: string;
}

export const Titlebar: React.FC<TitlebarProps> = ({ title = "Assistant" }) => {
  const isMacOS = useMemo(
    () => navigator.platform.toUpperCase().startsWith("MAC"),
    [],
  );
  const paddingLeft = isMacOS
    ? MACOS_TRAFFIC_LIGHT_OFFSET
    : TITLEBAR_PADDING_LEFT;

  return (
    <div
      data-tauri-drag-region
      style={
        {
          height: TITLEBAR_HEIGHT,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          paddingLeft,
          paddingRight: 4,
          borderBottom: "1px solid var(--color-border)",
          background: "var(--color-bg)",
          userSelect: "none",
          WebkitAppRegion: "drag",
        } as React.CSSProperties
      }
    >
      <div
        style={
          {
            display: "flex",
            alignItems: "center",
            gap: 8,
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties
        }
      >
        <span
          style={{
            fontSize: 13,
            fontWeight: 500,
            color: "var(--color-text-secondary)",
          }}
        >
          {title}
        </span>
      </div>

      <div
        style={
          {
            display: "flex",
            alignItems: "center",
            gap: 2,
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties
        }
      >
        <ThemeToggle />
        <WindowControls />
      </div>
    </div>
  );
};
