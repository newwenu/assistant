import { useState, useCallback } from "react";
import { Button, Space, Input } from "antd";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { mascotAssetUrl, isLocalAssetUrl } from "@/mascot/core/assetUrl";
import { settingService } from "../../../services/settingService";
import type { SettingFieldProps } from "../../core/types/fieldSchema";
import { useSettingMessage } from "../useSettingMessage";

export function AssetPickerField({
  schema,
  value,
  onChange,
}: SettingFieldProps) {
  const { t } = useTranslation();
  const msg = useSettingMessage();
  const config = schema.assetConfig;
  const ManifestEditor = config?.manifestEditor;
  const [urlInput, setUrlInput] = useState<string>((value as string) ?? "");

  const handleFilePick = useCallback(async () => {
    if (!config) return;
    const selected = await open({
      multiple: false,
      filters: [config.fileFilter],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const result = await settingService.installMascotAsset(
        config.assetType,
        selected,
      );
      const assetUrl = await mascotAssetUrl(result.relPath, result.version);
      onChange(assetUrl);
      setUrlInput(assetUrl);
      msg.success("settings.asset_picker.installed", {
        size: (result.bytes / 1024).toFixed(1),
      });
    } catch (e) {
      msg.error("settings.asset_picker.install_failed", e);
    }
  }, [config, onChange, msg]);

  const handleReset = useCallback(async () => {
    if (!config) return;
    try {
      await settingService.uninstallMascotAsset(config.assetType);
      onChange(null);
      setUrlInput("");
      msg.success("settings.asset_picker.reset_success");
    } catch (e) {
      msg.error("settings.asset_picker.reset_failed", e);
    }
  }, [config, onChange, msg]);

  const handleUrlSubmit = useCallback(() => {
    const trimmed = urlInput.trim();
    const next = trimmed.length === 0 ? null : trimmed;
    onChange(next);
  }, [urlInput, onChange]);

  if (!config) return null;

  return (
    <>
      <Space orientation="vertical" style={{ width: "100%" }}>
        <Space>
          <Button
            size="small"
            onClick={() => {
              void handleFilePick();
            }}
          >
            {t("settings.asset_picker.pick_file")}
          </Button>
          {value != null && (
            <Button
              size="small"
              danger
              onClick={() => {
                void handleReset();
              }}
            >
              {t("settings.asset_picker.reset_default")}
            </Button>
          )}
          {ManifestEditor &&
            typeof value === "string" &&
            isLocalAssetUrl(value) && (
              <ManifestEditor assetType={config.assetType} />
            )}
        </Space>
        {config.urlInput && (
          <Input
            placeholder={t("settings.asset_picker.url_placeholder")}
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            onBlur={handleUrlSubmit}
            onPressEnter={handleUrlSubmit}
          />
        )}
      </Space>
    </>
  );
}
