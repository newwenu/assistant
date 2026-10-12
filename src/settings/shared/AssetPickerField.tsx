import { useState, useCallback } from "react";
import { Button, Space, Input, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { SettingFieldProps } from "../core/fieldRegistry";

interface InstallAssetResult {
  installed_path: string;
  bytes: number;
  version: number;
}

export function AssetPickerField({ def, value, onChange }: SettingFieldProps) {
  const config = def.assetConfig;
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
      const result = (await invoke("install_mascot_asset", {
        assetType: config.assetType,
        filePath: selected,
      })) as InstallAssetResult;
      const assetUrl = `${convertFileSrc(result.installed_path)}?v=${result.version}`;
      onChange(assetUrl);
      setUrlInput(assetUrl);
      messageApi.success(`已安装文件 (${(result.bytes / 1024).toFixed(1)} KB)`);
    } catch (e) {
      messageApi.error(
        `安装失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [config, onChange, messageApi]);

  const handleReset = useCallback(async () => {
    if (!config) return;
    try {
      await invoke("uninstall_mascot_asset", {
        assetType: config.assetType,
      });
      onChange(null);
      setUrlInput("");
      messageApi.success("已重置为默认");
    } catch (e) {
      messageApi.error(
        `重置失败: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }, [config, onChange, messageApi]);

  const handleUrlSubmit = useCallback(async () => {
    const trimmed = urlInput.trim();
    const next = trimmed.length === 0 ? null : trimmed;
    onChange(next);
  }, [urlInput, onChange]);

  if (!config) return null;

  return (
    <>
      {contextHolder}
      <Space direction="vertical" style={{ width: "100%" }}>
        <Space>
          <Button size="small" onClick={handleFilePick}>
            选择文件
          </Button>
          {value != null && (
            <Button size="small" danger onClick={handleReset}>
              重置为默认
            </Button>
          )}
        </Space>
        {config.urlInput && (
          <Input
            placeholder="或输入 URL / 本地路径 / data URI"
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
