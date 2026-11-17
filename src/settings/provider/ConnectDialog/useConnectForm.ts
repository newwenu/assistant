import { useState } from "react";
import { Form } from "antd";

import type {
  ModelConfig,
  Provider,
  ProviderInput,
  ProviderPreset,
} from "../types";

interface UseConnectFormOptions {
  provider: Provider | null;
  preset: ProviderPreset | null;
  isPreset: boolean;
  /** 自定义 Provider 落库时的 type 值，由所选 format 决定 */
  customType: string;
  onSubmit: (input: ProviderInput) => void;
}

/**
 * 连接弹窗的表单状态：模型清单、保存中标志、打开时回填与提交装配。
 * 字段定义与渲染留在调用方，本 Hook 只负责数据生命周期。
 */
export function useConnectForm({
  provider,
  preset,
  isPreset,
  customType,
  onSubmit,
}: UseConnectFormOptions) {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const [models, setModels] = useState<ModelConfig[]>([]);

  const isEdit = !!provider;

  const handleOk = async () => {
    let values: Record<string, string>;
    try {
      values = (await form.validateFields()) as Record<string, string>;
    } catch {
      return;
    }
    setSaving(true);
    try {
      const input: ProviderInput = {
        id: provider?.id || "",
        name: values.name || (isPreset && preset ? preset.label : ""),
        type: isEdit
          ? provider.type
          : isPreset && preset
            ? preset.slug
            : values.type || customType,
        baseUrl:
          values.baseUrl || (isPreset && preset ? preset.defaultBaseUrl : ""),
        apiKey: values.apiKey || "",
        apiKeyChanged: !isEdit || !!values.apiKey,
        models,
        defaultModel: values.defaultModel || "",
      };
      onSubmit(input);
    } finally {
      setSaving(false);
    }
  };

  const handleAfterOpen = () => {
    if (provider) {
      form.setFieldsValue({
        name: provider.name,
        type: provider.type,
        baseUrl: provider.baseUrl,
        apiKey: "",
        defaultModel: provider.defaultModel,
      });
      setModels(provider.models ?? []);
    } else {
      form.setFieldsValue({
        name: isPreset && preset ? preset.label : "",
        type: isPreset && preset ? preset.slug : customType,
        baseUrl: isPreset && preset ? preset.defaultBaseUrl : "",
        apiKey: "",
        defaultModel: undefined,
      });
      setModels([]);
    }
  };

  return {
    form,
    saving,
    models,
    setModels,
    handleOk,
    handleAfterOpen,
  };
}
