import { useState, useCallback } from "react";
import { Button, Space, Input, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { SettingFieldProps } from "../core/fieldSchema";

interface InstallAssetResult {
  installed_path: string;
  bytes: number;
  version: number;
}

export function AssetPickerField({
  schema,
  value,
  onChange,
}: SettingFieldProps) {
  const { t } = useTranslation();
  const config = schema.assetConfig;
  const [urlInput, setUrlInput] = useState<string>((value as string) ?? "");
  const [messageApi, contextHolder] = message.useMessage();

  const handleFilePick = useCallback(async () => {
    if (!config) return;
    const selected = await open({
      multiple: false,
      filters: [config.fileFilter],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const result = await invoke<InstallAssetResult>("install_mascot_asset", {
        assetType: config.assetType,
        filePath: selected,
      });
      const assetUrl = `${convertFileSrc(result.installed_path)}?v=${result.version}`;
      onChange(assetUrl);
      setUrlInput(assetUrl);
      messageApi.success(
        t("settings.asset_picker.installed", {
          size: (result.bytes / 1024).toFixed(1),
        }),
      );
    } catch (e) {
      messageApi.error(
        t("settings.asset_picker.install_failed", {
          error: e instanceof Error ? e.message : String(e),
        }),
      );
    }
  }, [config, onChange, messageApi, t]);

  const handleReset = useCallback(async () => {
    if (!config) return;
    try {
      await invoke("uninstall_mascot_asset", {
        assetType: config.assetType,
      });
      onChange(null);
      setUrlInput("");
      messageApi.success(t("settings.asset_picker.reset_success"));
    } catch (e) {
      messageApi.error(
        t("settings.asset_picker.reset_failed", {
          error: e instanceof Error ? e.message : String(e),
        }),
      );
    }
  }, [config, onChange, messageApi, t]);

  const handleUrlSubmit = useCallback(() => {
    const trimmed = urlInput.trim();
    const next = trimmed.length === 0 ? null : trimmed;
    onChange(next);
  }, [urlInput, onChange]);

  if (!config) return null;

  return (
    <>
      {contextHolder}
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
