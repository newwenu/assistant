import { useMemo } from "react";
import { Button, Tooltip } from "antd";
import { ShrinkOutlined, ArrowsAltOutlined } from "@ant-design/icons";
import { ThemeToggle } from "./ThemeToggle";
import { WindowControls } from "./WindowControls";
import { PinToggle } from "./PinToggle";
import { SettingsEntry } from "./SettingsEntry";
import { translateKey } from "@/i18n";
import type { WindowMode } from "@/stores/system/windowModeStore";
import {
  TITLEBAR_HEIGHT,
  MINI_TITLEBAR_HEIGHT,
  MACOS_TRAFFIC_LIGHT_OFFSET,
  TITLEBAR_PADDING_LEFT,
} from "@/config/constants";

interface TitlebarProps {
  title?: string;
  mode?: WindowMode;
  onToggleMode?: () => void;
}

export const Titlebar: React.FC<TitlebarProps> = ({
  title = "Assistant",
  mode = "normal",
  onToggleMode,
}) => {
  const isMini = mode === "mini";
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
          height: isMini ? MINI_TITLEBAR_HEIGHT : TITLEBAR_HEIGHT,
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
        {onToggleMode && (
          <Tooltip
            title={translateKey(
              isMini ? "mini.toggle_to_normal" : "mini.toggle_to_mini",
            )}
            mouseEnterDelay={0.5}
          >
            <Button
              type="text"
              size="small"
              icon={isMini ? <ArrowsAltOutlined /> : <ShrinkOutlined />}
              onClick={onToggleMode}
              style={{ width: 32, height: 28 }}
            />
          </Tooltip>
        )}
        {!isMini && <PinToggle />}
        {!isMini && <SettingsEntry />}
        {!isMini && <ThemeToggle />}
        <WindowControls />
      </div>
    </div>
  );
};
