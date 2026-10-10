import { useEffect, useState, useCallback } from "react";
import {
  Select,
  ColorPicker,
  Button,
  Space,
  Typography,
  Switch,
  message,
} from "antd";
import type { AggregationColor } from "antd/es/color-picker/color";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  getAllRendererMetas,
  asRendererId,
  BUILTIN_RENDERER_IDS,
} from "@/mascot";
import { dispatchConfPatch } from "./settingsDispatcher";

const { Text } = Typography;

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

export const MascotSection: React.FC = () => {
  const mascotConf = useSettingsStore((s) => s.mascotConf);
  const metas = getAllRendererMetas();

  const currentRendererId = mascotConf?.renderer ?? "rive";
  const currentMeta = metas.find(
    (m) => m.id === asRendererId(currentRendererId),
  );
  const capabilities = currentMeta?.capabilities;
  const defaultColors = currentMeta?.defaultColors;

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

  const handleRendererChange = useCallback(async (value: string) => {
    await dispatchConfPatch("mascot", { renderer: value });
  }, []);

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

  const handleApplyColors = useCallback(async () => {
    try {
      setApplying(true);
      await dispatchConfPatch("mascot", {
        primary_color: primaryColor,
        secondary_color: secondaryColor,
      });
      setDirty(false);
      messageApi.success("颜色已应用");
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      messageApi.error(`应用失败: ${errMsg}`);
    } finally {
      setApplying(false);
    }
  }, [primaryColor, secondaryColor, messageApi]);

  const handleResetColors = useCallback(() => {
    setPrimaryColor(null);
    setSecondaryColor(null);
    setDirty(
      !colorsEqual(null, mascotConf?.primary_color) ||
        !colorsEqual(null, mascotConf?.secondary_color),
    );
  }, [mascotConf]);

  const handleIdlePoseRotationChange = useCallback(async (checked: boolean) => {
    await dispatchConfPatch("mascot", { rive_idle_pose_rotation: checked });
  }, []);

  const handleSvgAnimatedChange = useCallback(async (checked: boolean) => {
    await dispatchConfPatch("mascot", { svg_animated: checked });
  }, []);

  return (
    <>
      {contextHolder}
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div>
          <Text type="secondary" style={{ display: "block", marginBottom: 4 }}>
            渲染方式
          </Text>
          <Select
            value={currentRendererId}
            onChange={handleRendererChange}
            style={{ width: "100%" }}
            options={metas.map((m) => ({
              value: m.id as string,
              label: m.label,
            }))}
          />
        </div>

        {currentMeta?.limitations && (
          <Text type="warning">{currentMeta.limitations}</Text>
        )}

        {capabilities?.colorCustomization && defaultColors && (
          <div>
            <div style={{ marginBottom: 8, fontSize: 14 }}>主色</div>
            <Space>
              <ColorPicker
                size="small"
                value={primaryColor ?? undefined}
                onChange={handlePrimaryChange}
              />
              {primaryColor && (
                <span
                  style={{
                    fontSize: 12,
                    color: "var(--text-tertiary)",
                  }}
                >
                  {primaryColor}
                </span>
              )}
            </Space>

            {defaultColors.secondary != null && (
              <>
                <div style={{ marginTop: 12, marginBottom: 8, fontSize: 14 }}>
                  辅色
                </div>
                <Space>
                  <ColorPicker
                    size="small"
                    value={secondaryColor ?? undefined}
                    onChange={handleSecondaryChange}
                  />
                  {secondaryColor && (
                    <span
                      style={{
                        fontSize: 12,
                        color: "var(--text-tertiary)",
                      }}
                    >
                      {secondaryColor}
                    </span>
                  )}
                </Space>
              </>
            )}

            <div style={{ marginTop: 12 }}>
              <Space>
                <Button
                  type="primary"
                  size="small"
                  disabled={!dirty}
                  loading={applying}
                  onClick={handleApplyColors}
                >
                  应用
                </Button>
                <Button size="small" onClick={handleResetColors}>
                  重置
                </Button>
              </Space>
            </div>
          </div>
        )}

        {currentRendererId === BUILTIN_RENDERER_IDS.rive && (
          <div>
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 4 }}
            >
              空闲轮转动画
            </Text>
            <Switch
              checked={mascotConf?.rive_idle_pose_rotation ?? false}
              onChange={handleIdlePoseRotationChange}
            />
          </div>
        )}

        {currentRendererId === BUILTIN_RENDERER_IDS.svg && (
          <div>
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 4 }}
            >
              SVG 动画
            </Text>
            <Switch
              checked={mascotConf?.svg_animated ?? true}
              onChange={handleSvgAnimatedChange}
            />
          </div>
        )}
      </div>
    </>
  );
};
