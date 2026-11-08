import { useState, useEffect, useCallback } from "react";
import { ColorPicker, Button, Space } from "antd";
import type { AggregationColor } from "antd/es/color-picker/color";
import { useTranslation } from "react-i18next";
import type { FieldRenderProps } from "../../core/types/fieldSchema";
import { dispatchConfPatch } from "../../core/settingsDispatcher";
import { getAllRendererMetas, asRendererId } from "@/mascot";
import { useSettingMessage } from "../useSettingMessage";

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

export function MascotColorGroupRender({ value, conf }: FieldRenderProps) {
  const { t } = useTranslation();
  const rendererId = (conf?.renderer as string) ?? "svg";
  const metas = getAllRendererMetas();
  const currentMeta = metas.find((m) => m.id === asRendererId(rendererId));
  const capabilities = currentMeta?.capabilities;
  const defaultColors = currentMeta?.defaultColors;
  const showSecondary = defaultColors?.secondary != null;

  const confPrimary = (conf?.primaryColor as string) ?? null;
  const confSecondary = (conf?.secondaryColor as string) ?? null;

  const [primaryColor, setPrimaryColor] = useState<string | null>(
    (value as string) ?? null,
  );
  const [secondaryColor, setSecondaryColor] = useState<string | null>(
    confSecondary,
  );
  const [dirty, setDirty] = useState(false);
  const [applying, setApplying] = useState(false);
  const msg = useSettingMessage();

  useEffect(() => {
    setPrimaryColor(confPrimary);
    setSecondaryColor(confSecondary);
    setDirty(false);
  }, [confPrimary, confSecondary]);

  const handlePrimaryChange = useCallback(
    (_: AggregationColor) => {
      const hex = toHex(_);
      setPrimaryColor(hex);
      setDirty(
        !colorsEqual(hex, confPrimary) ||
          !colorsEqual(secondaryColor, confSecondary),
      );
    },
    [confPrimary, confSecondary, secondaryColor],
  );

  const handleSecondaryChange = useCallback(
    (_: AggregationColor) => {
      const hex = toHex(_);
      setSecondaryColor(hex);
      setDirty(
        !colorsEqual(primaryColor, confPrimary) ||
          !colorsEqual(hex, confSecondary),
      );
    },
    [confPrimary, confSecondary, primaryColor],
  );

  const handleApply = useCallback(async () => {
    try {
      setApplying(true);
      await dispatchConfPatch("mascot", {
        primaryColor: primaryColor,
        secondaryColor: secondaryColor,
      });
      setDirty(false);
      msg.success("settings.mascot.color.applied");
    } catch (e) {
      msg.error("settings.mascot.color.apply_failed", e);
    } finally {
      setApplying(false);
    }
  }, [primaryColor, secondaryColor, msg]);

  const handleReset = useCallback(() => {
    setPrimaryColor(null);
    setSecondaryColor(null);
    setDirty(
      !colorsEqual(null, confPrimary) || !colorsEqual(null, confSecondary),
    );
  }, [confPrimary, confSecondary]);

  if (!capabilities?.colorCustomization || !defaultColors) return null;

  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 24 }}>
          <div>
            <div style={{ marginBottom: 4, fontSize: 14 }}>
              {t("settings.mascot.color.primary")}
            </div>
            <Space>
              <ColorPicker
                size="small"
                value={primaryColor ?? undefined}
                onChange={handlePrimaryChange}
              />
              {primaryColor && (
                <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
                  {primaryColor}
                </span>
              )}
            </Space>
          </div>
          {showSecondary && (
            <div>
              <div style={{ marginBottom: 4, fontSize: 14 }}>
                {t("settings.mascot.color.secondary")}
              </div>
              <Space>
                <ColorPicker
                  size="small"
                  value={secondaryColor ?? undefined}
                  onChange={handleSecondaryChange}
                />
                {secondaryColor && (
                  <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
                    {secondaryColor}
                  </span>
                )}
              </Space>
            </div>
          )}
        </div>
        <div style={{ marginTop: 4 }}>
          <Space>
            <Button
              type="primary"
              size="small"
              disabled={!dirty}
              loading={applying}
              onClick={() => {
                void handleApply();
              }}
            >
              {t("settings.mascot.color.apply")}
            </Button>
            <Button size="small" onClick={handleReset}>
              {t("settings.mascot.color.reset")}
            </Button>
          </Space>
        </div>
      </div>
    </>
  );
}
