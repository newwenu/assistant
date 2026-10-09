import { useEffect, useState, useCallback } from "react";
import { ColorPicker, Button, Space, message } from "antd";
import type { AggregationColor } from "antd/es/color-picker/color";
import { useSettingsStore } from "@/stores/settingsStore";
import { dispatchConfPatch } from "./settingsDispatcher";

interface MascotColorSectionProps {
  onSaved?: () => void;
}

function toHex(
  color: AggregationColor | string | null | undefined,
): string | null {
  if (color == null) return null;
  if (typeof color === "string") {
    if (color.startsWith("#") && color.length === 7) return color;
    return null;
  }
  try {
    const hex = color.toHexString();
    return hex.startsWith("#") && hex.length === 7 ? hex : null;
  } catch {
    return null;
  }
}

function colorsEqual(
  a: string | null | undefined,
  b: string | null | undefined,
): boolean {
  if (a == null && b == null) return true;
  return a === b;
}

export const MascotColorSection: React.FC<MascotColorSectionProps> = ({
  onSaved,
}) => {
  const mascotConf = useSettingsStore((s) => s.mascotConf);
  const [primaryColor, setPrimaryColor] = useState<string | null>(
    mascotConf?.primary_color ?? null,
  );
  const [secondaryColor, setSecondaryColor] = useState<string | null>(
    mascotConf?.secondary_color ?? null,
  );
  const [dirty, setDirty] = useState(false);
  const [applying, setApplying] = useState(false);
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    if (!mascotConf) return;
    setPrimaryColor(mascotConf.primary_color ?? null);
    setSecondaryColor(mascotConf.secondary_color ?? null);
    setDirty(false);
  }, [mascotConf]);

  const handlePrimaryChange = useCallback(
    (_: AggregationColor) => {
      const hex = toHex(_);
      setPrimaryColor(hex);
      setDirty(
        !colorsEqual(hex, mascotConf?.primary_color) ||
          !colorsEqual(secondaryColor, mascotConf?.secondary_color),
      );
    },
    [mascotConf, secondaryColor],
  );

  const handleSecondaryChange = useCallback(
    (_: AggregationColor) => {
      const hex = toHex(_);
      setSecondaryColor(hex);
      setDirty(
        !colorsEqual(primaryColor, mascotConf?.primary_color) ||
          !colorsEqual(hex, mascotConf?.secondary_color),
      );
    },
    [mascotConf, primaryColor],
  );

  const handleApply = useCallback(async () => {
    try {
      setApplying(true);
      const patch: Record<string, unknown> = {
        primary_color: primaryColor,
        secondary_color: secondaryColor,
      };
      await dispatchConfPatch("mascot", patch);
      setDirty(false);
      onSaved?.();
      messageApi.success("颜色已应用");
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      messageApi.error(`应用失败: ${errMsg}`);
    } finally {
      setApplying(false);
    }
  }, [primaryColor, secondaryColor, onSaved, messageApi]);

  const handleReset = useCallback(() => {
    setPrimaryColor(null);
    setSecondaryColor(null);
    setDirty(
      !colorsEqual(null, mascotConf?.primary_color) ||
        !colorsEqual(null, mascotConf?.secondary_color),
    );
  }, [mascotConf]);

  return (
    <>
      {contextHolder}
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div>
          <div style={{ marginBottom: 4, fontSize: 14 }}>主色</div>
          <ColorPicker
            size="small"
            value={primaryColor ?? undefined}
            onChange={handlePrimaryChange}
          />
          {primaryColor && (
            <span
              style={{
                marginLeft: 8,
                fontSize: 12,
                color: "var(--text-tertiary)",
              }}
            >
              {primaryColor}
            </span>
          )}
        </div>
        <div>
          <div style={{ marginBottom: 4, fontSize: 14 }}>辅色</div>
          <ColorPicker
            size="small"
            value={secondaryColor ?? undefined}
            onChange={handleSecondaryChange}
          />
          {secondaryColor && (
            <span
              style={{
                marginLeft: 8,
                fontSize: 12,
                color: "var(--text-tertiary)",
              }}
            >
              {secondaryColor}
            </span>
          )}
        </div>
        <Space>
          <Button
            type="primary"
            size="small"
            disabled={!dirty}
            loading={applying}
            onClick={handleApply}
          >
            应用
          </Button>
          <Button size="small" onClick={handleReset}>
            重置
          </Button>
        </Space>
      </div>
    </>
  );
};
