import React, { useMemo, useState } from "react";
import { Modal, Form, Button, Typography, message } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  type Provider,
  type ProviderPreset,
  type ModelConfig,
  type ApiFormat,
  getApiFormatByType,
  getFormatInfo,
} from "@/stores/providerStore";
import { getProviderPreset } from "../core/providerPresetRegistry";
import { getProviderFormatLabel } from "../core/providerFormatRegistry";
import type { FieldSchema, FieldContext } from "../core/fieldSchema";
import { SettingField } from "../shared/SettingField";
import { FormatTag } from "./FormatTag";

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
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [messageApi, contextHolder] = message.useMessage();

  const isEdit = !!provider;
  const effectiveFormat = isEdit
    ? getApiFormatByType(provider.type)
    : (preset?.format ?? customFormat ?? "openai");
  const formatInfo = getFormatInfo(effectiveFormat);
  const isPreset = isEdit ? !!getProviderPreset(provider.type) : !!preset;
  const isLocal = effectiveFormat === "local";
  const showType = !isEdit && !isPreset;

  const formValues = Form.useWatch([], form) as
    Record<string, unknown> | undefined;

  const fieldContext = useMemo<FieldContext>(
    () => ({
      mode: isEdit ? "edit" : "add",
      formValues: formValues ?? {},
      entity: provider ?? undefined,
      preset: preset ?? undefined,
    }),
    [isEdit, formValues, provider, preset],
  );

  const fieldSchemas = useMemo<FieldSchema[]>(
    () => [
      {
        key: "name",
        type: "string",
        label: t("provider.field.name.label"),
        placeholder: (ctx) =>
          ctx.mode === "add" && ctx.preset
            ? (ctx.preset as ProviderPreset).label
            : t("provider.field.name.placeholder"),
      },
      {
        key: "type",
        type: "string",
        label: t("provider.field.type.label"),
        placeholder: t("provider.field.type.placeholder"),
        visible: () => showType,
      },
      {
        key: "api_key",
        type: "secret",
        label: t("provider.field.api_key.label"),
        description: (ctx) =>
          ctx.mode === "edit"
            ? t("provider.field.api_key.keep_hint")
            : t("provider.field.api_key.encrypted_hint"),
        placeholder: (ctx) =>
          ctx.mode === "edit"
            ? (ctx.entity as Provider)?.api_key_masked ||
              t("provider.field.api_key.keep")
            : formatInfo?.keyPlaceholder || "sk-...",
        visible: () => !isLocal,
        keepIfEmpty: isEdit,
      },
      {
        key: "base_url",
        type: "string",
        label: t("provider.field.base_url.label"),
        placeholder: (ctx) =>
          ctx.preset
            ? (ctx.preset as ProviderPreset).defaultBaseUrl
            : "https://...",
      },
      {
        key: "default_model",
        type: "select",
        label: t("provider.field.default_model.label"),
        allowClear: true,
        placeholder: t("provider.field.default_model.placeholder"),
        options: models.map((m) => ({
          label: m.name || m.id,
          value: m.id,
        })),
        actions: [
          {
            key: "fetch_models",
            label: t("provider.field.fetch_models"),
            onClick: async (ctx, callbacks) => {
              const fv = ctx.formValues;
              const baseUrl =
                (fv.base_url as string) ||
                (ctx.preset
                  ? (ctx.preset as ProviderPreset).defaultBaseUrl
                  : "");
              const apiKey = (fv.api_key as string) || "";
              if (!apiKey && !isLocal && ctx.mode === "add") {
                messageApi.warning(t("provider.field.api_key.required_first"));
                return;
              }
              try {
                let result: unknown;
                if (ctx.mode === "edit" && !apiKey) {
                  result = await invoke("list_models_cmd", {
                    providerId: (ctx.entity as Provider)?.id,
                  });
                } else {
                  result = await invoke("preview_models_cmd", {
                    providerType:
                      ctx.mode === "edit"
                        ? (ctx.entity as Provider)?.type
                        : ctx.preset
                          ? (ctx.preset as ProviderPreset).slug
                          : "custom",
                    baseUrl,
                    apiKey,
                  });
                }
                const modelList = (result as ModelConfig[]) || [];
                setModels(modelList);
                callbacks.setFieldOptions(
                  modelList.map((m) => ({
                    label: m.name || m.id,
                    value: m.id,
                  })),
                );
                messageApi.success(
                  t("provider.field.models_fetched", {
                    count: modelList.length,
                  }),
                );
              } catch (e) {
                messageApi.error(
                  t("provider.field.models_fetch_failed", {
                    error: e instanceof Error ? e.message : String(e),
                  }),
                );
              }
            },
          },
        ],
      },
    ],
    [t, showType, isLocal, isEdit, formatInfo, models, messageApi],
  );

  const getTitle = () => {
    if (isEdit) return t("provider.edit_title", { name: provider.name });
    if (preset) return t("provider.connect_title", { name: preset.label });
    return t("provider.add_custom_title", {
      format: t(getProviderFormatLabel(effectiveFormat)),
    });
  };

  const getDescription = () => {
    if (isLocal) return t("provider.description.local");
    return undefined;
  };

  const handleOk = async () => {
    let values: Record<string, string>;
    try {
      values = (await form.validateFields()) as Record<string, string>;
    } catch {
      return;
    }
    setSaving(true);
    try {
      const input: Record<string, unknown> = {
        id: provider?.id || "",
        name: values.name || (isPreset && preset ? preset.label : ""),
        type: isEdit
          ? provider.type
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

  const handleAfterOpen = () => {
    if (isEdit) {
      form.setFieldsValue({
        name: provider.name,
        type: provider.type,
        base_url: provider.base_url,
        api_key: "",
        default_model: provider.default_model,
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

  const shouldShowField = (schema: FieldSchema): boolean => {
    if (!schema.visible) return true;
    return schema.visible(formValues ?? {});
  };

  return (
    <Modal
      title={
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span>{getTitle()}</span>
          <FormatTag
            type={
              isEdit
                ? provider.type
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
          <Button onClick={onClose}>{t("actions.cancel")}</Button>
          <Button
            type="primary"
            loading={saving}
            onClick={() => {
              void handleOk();
            }}
          >
            {isEdit ? t("actions.save") : t("provider.connect")}
          </Button>
        </div>
      }
      width="var(--modal-width-form)"
    >
      {contextHolder}
      <Form form={form} layout="vertical" style={{ marginTop: 16 }}>
        <Typography.Paragraph
          type="secondary"
          style={{ fontSize: 12, marginBottom: 12 }}
        >
          {getDescription()}
        </Typography.Paragraph>

        {fieldSchemas.filter(shouldShowField).map((schema) => {
          const fieldExtra =
            schema.description !== undefined
              ? typeof schema.description === "function"
                ? schema.description(fieldContext)
                : schema.description
              : undefined;
          return (
            <Form.Item
              key={schema.key}
              name={schema.key}
              label={schema.label}
              extra={
                fieldExtra ? (
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    {fieldExtra}
                  </Typography.Text>
                ) : undefined
              }
              rules={
                schema.key === "name"
                  ? [
                      {
                        required: true,
                        message: t("provider.field.name.required"),
                      },
                    ]
                  : schema.key === "base_url"
                    ? [
                        {
                          required: true,
                          message: t("provider.field.base_url.required"),
                        },
                      ]
                    : schema.key === "api_key" && !isEdit && !isLocal
                      ? [
                          {
                            required: true,
                            message: t("provider.field.api_key.required"),
                          },
                        ]
                      : schema.key === "type" && showType
                        ? [
                            {
                              required: true,
                              message: t("provider.field.type.required"),
                            },
                          ]
                        : undefined
              }
            >
              <SettingField
                schema={schema}
                value={null}
                onChange={() => {}}
                fieldContext={fieldContext}
              />
            </Form.Item>
          );
        })}

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
                {t("provider.field.models_more", {
                  count: models.length - 20,
                })}
              </Typography.Text>
            )}
          </div>
        )}
      </Form>
    </Modal>
  );
};
