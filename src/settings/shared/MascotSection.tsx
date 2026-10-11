import { useEffect, useState, useCallback } from "react";
import {
  Select,
  ColorPicker,
  Button,
  Space,
  Typography,
  Switch,
  Input,
  Form,
  message,
} from "antd";
import type { AggregationColor } from "antd/es/color-picker/color";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  getAllRendererMetas,
  asRendererId,
  BUILTIN_RENDERER_IDS,
} from "@/mascot";
import { dispatchConfPatch } from "../core/settingsDispatcher";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";
import { SettingsGroup } from "./SettingsGroup";
import { MASCOT_GIF_SETTINGS } from "../core/fieldDefinitions";

const { Text } = Typography;

interface InstallAssetResult {
  installed_path: string;
  bytes: number;
  version: number;
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
  const [gifSrcInput, setGifSrcInput] = useState<string>(
    mascotConf?.gif_src ?? "",
  );
  const [messageApi, contextHolder] = message.useMessage();

  const { onPatch: mascotPatch, onPatchComplete: mascotPatchComplete } =
    useConfPatchHandlers("mascot");

  useEffect(() => {
    if (!mascotConf) return;
    setPrimaryColor(mascotConf.primary_color ?? null);
    setSecondaryColor(mascotConf.secondary_color ?? null);
    setGifSrcInput(mascotConf.gif_src ?? "");
    setDirty(false);
  }, [mascotConf]);

  const handleRendererChange = useCallback(
    async (value: string) => {
      await mascotPatch({ renderer: value });
      messageApi.success("渲染方式已切换");
    },
    [mascotPatch, messageApi],
  );

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

  const handleSwitchPatch = useCallback(
    (key: string, checked: boolean) => {
      mascotPatch({ [key]: checked });
      messageApi.success("已保存");
    },
    [mascotPatch, messageApi],
  );

  const handleGifSrcSubmit = useCallback(async () => {
    const trimmed = gifSrcInput.trim();
    const next = trimmed.length === 0 ? null : trimmed;
    if (next === mascotConf?.gif_src) return;
    try {
      await dispatchConfPatch("mascot", { gif_src: next });
      messageApi.success("GIF 来源已更新");
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      messageApi.error(`更新失败: ${errMsg}`);
    }
  }, [gifSrcInput, mascotConf, messageApi]);

  const handleGifFilePick = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["gif", "png", "webp"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const result = (await invoke("install_mascot_asset", {
        assetType: "gif",
        filePath: selected,
      })) as InstallAssetResult;
      const assetUrl = `${convertFileSrc(result.installed_path)}?v=${result.version}`;
      await dispatchConfPatch("mascot", { gif_src: assetUrl });
      messageApi.success(`已安装图片 (${(result.bytes / 1024).toFixed(1)} KB)`);
    } catch (e) {
      messageApi.error(
        `安装失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [messageApi]);

  const handleGifSrcReset = useCallback(async () => {
    try {
      await invoke("uninstall_mascot_asset", { assetType: "gif" });
      await dispatchConfPatch("mascot", { gif_src: null });
      setGifSrcInput("");
      messageApi.success("已重置为默认");
    } catch (e) {
      messageApi.error(
        `重置失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [messageApi]);

  const handleRiveFilePick = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Rive", extensions: ["riv"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const result = (await invoke("install_mascot_asset", {
        assetType: "rive",
        filePath: selected,
      })) as InstallAssetResult;
      const assetUrl = `${convertFileSrc(result.installed_path)}?v=${result.version}`;
      await dispatchConfPatch("mascot", { rive_src: assetUrl });
      messageApi.success(
        `已安装 Rive 文件 (${(result.bytes / 1024).toFixed(1)} KB)`,
      );
    } catch (e) {
      messageApi.error(
        `安装失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [messageApi]);

  const handleRiveSrcReset = useCallback(async () => {
    try {
      await invoke("uninstall_mascot_asset", { assetType: "rive" });
      await dispatchConfPatch("mascot", { rive_src: null });
      messageApi.success("已重置为默认");
    } catch (e) {
      messageApi.error(
        `重置失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [messageApi]);

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
              onChange={(c) => handleSwitchPatch("rive_idle_pose_rotation", c)}
            />
            <div style={{ marginTop: 12 }}>
              <Text
                type="secondary"
                style={{ display: "block", marginBottom: 4 }}
              >
                自定义 Rive 文件
              </Text>
              <Space>
                <Button size="small" onClick={handleRiveFilePick}>
                  选择文件
                </Button>
                {mascotConf?.rive_src && (
                  <Button size="small" danger onClick={handleRiveSrcReset}>
                    重置为默认
                  </Button>
                )}
              </Space>
              <Text
                type="secondary"
                style={{ fontSize: 12, display: "block", marginTop: 4 }}
              >
                支持 .riv 文件，替换内置 mascot
              </Text>
            </div>
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
              onChange={(c) => handleSwitchPatch("svg_animated", c)}
            />
          </div>
        )}

        {currentRendererId === BUILTIN_RENDERER_IDS.gif && (
          <>
            <Form layout="vertical">
              <SettingsGroup
                group="mascot"
                definitions={MASCOT_GIF_SETTINGS}
                onPatch={mascotPatch}
                onPatchComplete={mascotPatchComplete}
              />
            </Form>
            <div>
              <Text
                type="secondary"
                style={{ display: "block", marginBottom: 4 }}
              >
                GIF 来源
              </Text>
              <Space direction="vertical" style={{ width: "100%" }}>
                <Space>
                  <Button size="small" onClick={handleGifFilePick}>
                    选择文件
                  </Button>
                  {mascotConf?.gif_src && (
                    <Button size="small" danger onClick={handleGifSrcReset}>
                      重置为默认
                    </Button>
                  )}
                </Space>
                <Input
                  placeholder="或输入 URL / 本地路径 / data URI"
                  value={gifSrcInput}
                  onChange={(e) => setGifSrcInput(e.target.value)}
                  onBlur={handleGifSrcSubmit}
                  onPressEnter={handleGifSrcSubmit}
                />
              </Space>
              <Text
                type="secondary"
                style={{ fontSize: 12, display: "block", marginTop: 4 }}
              >
                支持 GIF/PNG/WebP 文件上传，或 URL、本地路径、data URI
              </Text>
            </div>
          </>
        )}
      </div>
    </>
  );
};