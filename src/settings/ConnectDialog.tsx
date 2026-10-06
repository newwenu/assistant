import React, { useState } from "react";
import { Modal, Input, Form, Button, Typography, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import {
  type Provider,
  type ProviderPreset,
  type ModelConfig,
  type ApiFormat,
  API_FORMAT_MAP,
  getApiFormatByType,
  PRESET_MAP,
} from "../stores/providerStore";
import { FormatTag } from "./ProviderSwatch";

interface ConnectDialogProps {
  provider: Provider | null;
  preset: ProviderPreset | null;
  customFormat: ApiFormat | null;
  open: boolean;
  onClose: () => void;
  onSubmit: (input: Record<string, unknown>) => void;
}

export const ConnectDialog: React.FC<ConnectDialogProps> = ({
  provider,
  preset,
  customFormat,
  open,
  onClose,
  onSubmit,
}) => {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);

  const isEdit = !!provider;
  const effectiveFormat = isEdit
    ? getApiFormatByType(provider!.type)
    : (preset?.format ?? customFormat ?? "openai");
  const formatInfo = API_FORMAT_MAP.get(effectiveFormat);
  const isPreset = isEdit ? !!PRESET_MAP.get(provider!.type) : !!preset;
  const isLocal = effectiveFormat === "local";

  const getTitle = () => {
    if (isEdit) return `编辑 ${provider!.name}`;
    if (preset) return `连接 ${preset.label}`;
    return `添加自定义 ${formatInfo?.label ?? ""} Provider`;
  };

  const handleOk = async () => {
    let values: Record<string, string>;
    try {
      values = await form.validateFields();
    } catch {
      return;
    }
    setSaving(true);
    try {
      const input: Record<string, unknown> = {
        id: provider?.id || "",
        name: values.name || (isPreset && preset ? preset.label : ""),
        type: isEdit
          ? provider!.type
          : isPreset && preset
            ? preset.slug
            : values.type || "custom",
        base_url:
          values.base_url || (isPreset && preset ? preset.defaultBaseUrl : ""),
        api_key: values.api_key || "",
        models: JSON.stringify(models),
        default_model: values.default_model || "",
      };
      onSubmit(input);
    } finally {
      setSaving(false);
    }
  };

  const handleFetchModels = async () => {
    const baseUrl =
      (form.getFieldValue("base_url") as string) ||
      (isPreset && preset ? preset.defaultBaseUrl : "");
    const apiKey = (form.getFieldValue("api_key") as string) || "";
    if (!apiKey && !isLocal && !isEdit) {
      message.warning("请先输入 API Key");
      return;
    }
    setFetchingModels(true);
    try {
      let result: unknown;
      if (isEdit && !apiKey) {
        result = await invoke("list_models_cmd", { providerId: provider!.id });
      } else {
        result = await invoke("preview_models_cmd", {
          providerType: isEdit
            ? provider!.type
            : isPreset && preset
              ? preset.slug
              : "custom",
          baseUrl,
          apiKey,
        });
      }
      setModels((result as ModelConfig[]) || []);
      message.success(
        `获取到 ${((result as ModelConfig[]) || []).length} 个模型`,
      );
    } catch (e) {
      message.error(`获取模型列表失败: ${e}`);
    } finally {
      setFetchingModels(false);
    }
  };

  const handleAfterOpen = () => {
    if (isEdit) {
      form.setFieldsValue({
        name: provider!.name,
        type: provider!.type,
        base_url: provider!.base_url,
        api_key: "",
        default_model: provider!.default_model,
      });
    } else {
      form.setFieldsValue({
        name: isPreset && preset ? preset.label : "",
        type: isPreset && preset ? preset.slug : "",
        base_url: isPreset && preset ? preset.defaultBaseUrl : "",
        api_key: "",
        default_model: undefined,
      });
    }
    setModels([]);
  };

  const showType = !isEdit && !isPreset;

  return (
    <Modal
      title={
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span>{getTitle()}</span>
          <FormatTag
            type={
              isEdit
                ? provider!.type
                : isPreset && preset
                  ? preset.slug
                  : effectiveFormat
            }
            size="small"
          />
        </div>
      }
      open={open}
      onCancel={onClose}
      afterOpenChange={(v) => {
        if (v) handleAfterOpen();
      }}
      footer={
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <Button onClick={onClose}>取消</Button>
          <Button type="primary" loading={saving} onClick={handleOk}>
            {isEdit ? "保存" : "连接"}
          </Button>
        </div>
      }
      width={440}
    >
      <Form form={form} layout="vertical" style={{ marginTop: 16 }}>
        <Typography.Paragraph
          type="secondary"
          style={{ fontSize: 12, marginBottom: 12 }}
        >
          {isLocal
            ? "输入端点地址，确保服务已在本地运行。"
            : isEdit
              ? "修改后保存即可生效。留空 API Key 则保留原密钥。"
              : "输入 API Key，密钥将加密存储。"}
        </Typography.Paragraph>

        <Form.Item
          name="name"
          label="名称"
          rules={[{ required: true, message: "请输入名称" }]}
        >
          <Input placeholder="如 My Gateway" />
        </Form.Item>

        {showType && (
          <Form.Item
            name="type"
            label="Provider ID"
            rules={[{ required: true, message: "请输入 ID" }]}
          >
            <Input placeholder="小写英文，如 my-gateway" />
          </Form.Item>
        )}

        {!isLocal && (
          <Form.Item
            name="api_key"
            label="API Key"
            rules={
              isEdit ? [] : [{ required: true, message: "请输入 API Key" }]
            }
          >
            <Input.Password
              placeholder={
                isEdit
                  ? provider?.api_key_masked || "保留原密钥"
                  : formatInfo?.keyPlaceholder || "sk-..."
              }
            />
          </Form.Item>
        )}

        <Form.Item
          name="base_url"
          label="Base URL"
          rules={[{ required: true, message: "请输入 Base URL" }]}
        >
          <Input
            placeholder={
              isPreset && preset ? preset.defaultBaseUrl : "https://..."
            }
          />
        </Form.Item>

        <Form.Item label="默认模型">
          <div style={{ display: "flex", gap: 8 }}>
            <Form.Item name="default_model" noStyle>
              <Input
                placeholder="获取模型后选择或手动输入"
                style={{ flex: 1 }}
              />
            </Form.Item>
            <Button onClick={handleFetchModels} loading={fetchingModels}>
              获取模型
            </Button>
          </div>
          {models.length > 0 && (
            <div className="connect-model-list">
              {models.slice(0, 20).map((m) => (
                <button
                  key={m.id}
                  type="button"
                  className="connect-model-item"
                  onClick={() => form.setFieldsValue({ default_model: m.id })}
                >
                  <span className="connect-model-name">{m.name || m.id}</span>
                  <span className="connect-model-id">{m.id}</span>
                </button>
              ))}
              {models.length > 20 && (
                <Typography.Text
                  type="secondary"
                  style={{ fontSize: 11, padding: "4px 0" }}
                >
                  还有 {models.length - 20} 个模型...
                </Typography.Text>
              )}
            </div>
          )}
        </Form.Item>
      </Form>
    </Modal>
  );
};