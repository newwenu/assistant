import React, { useMemo, useState } from "react";
import { Modal, Form, Button, Typography } from "antd";
import { providerService } from "@/services/providerService";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import {
  getApiFormatByType,
  getFormatInfo,
} from "@/settings/core/provider/providerLookup";
import type {
  Provider,
  ProviderInput,
  ProviderPreset,
  ModelConfig,
  ApiFormat,
} from "./types";
import { getProviderPreset } from "../core/provider/providerPresetRegistry";
import { getProviderFormatLabel } from "../core/provider/providerFormatRegistry";
import type { FieldSchema, FieldContext } from "../core/types/fieldSchema";
import { SettingField } from "../shared/renderers/SettingField";
import { FormatTag } from "./FormatTag";
import { useSettingMessage } from "../shared/useSettingMessage";

interface ConnectDialogProps {
  provider: Provider | null;
  preset: ProviderPreset | null;
  customFormat: ApiFormat | null;
  open: boolean;
  onClose: () => void;
  onSubmit: (input: ProviderInput) => void;
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
  const msg = useSettingMessage();

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
        label: t("settings.provider.field.name.label"),
        placeholder: (ctx) =>
          ctx.mode === "add" && ctx.preset
            ? (ctx.preset as ProviderPreset).label
            : t("settings.provider.field.name.placeholder"),
      },
      {
        key: "type",
        type: "string",
        label: t("settings.provider.field.type.label"),
        placeholder: t("settings.provider.field.type.placeholder"),
        visible: () => showType,
      },
      {
        key: "apiKey",
        type: "secret",
        label: t("settings.provider.field.api_key.label"),
        description: (ctx) =>
          ctx.mode === "edit"
            ? t("settings.provider.field.api_key.keep_hint")
            : t("settings.provider.field.api_key.encrypted_hint"),
        placeholder: (ctx) =>
          ctx.mode === "edit"
            ? (ctx.entity as Provider)?.apiKeyMasked ||
              t("settings.provider.field.api_key.keep")
            : formatInfo?.keyPlaceholder || "sk-...",
        visible: () => !isLocal,
        keepIfEmpty: isEdit,
      },
      {
        key: "baseUrl",
        type: "string",
        label: t("settings.provider.field.base_url.label"),
        placeholder: (ctx) =>
          ctx.preset
            ? (ctx.preset as ProviderPreset).defaultBaseUrl
            : "https://...",
      },
      {
        key: "defaultModel",
        type: "select",
        label: t("settings.provider.field.default_model.label"),
        allowClear: true,
        placeholder: t("settings.provider.field.default_model.placeholder"),
        options: models.map((m) => ({
          label: m.name || m.id,
          value: m.id,
        })),
        actions: [
          {
            key: "fetch_models",
            label: t("settings.provider.field.fetch_models"),
            onClick: async (ctx, callbacks) => {
              const fv = ctx.formValues;
              const baseUrl =
                (fv.baseUrl as string) ||
                (ctx.preset
                  ? (ctx.preset as ProviderPreset).defaultBaseUrl
                  : "");
              const apiKey = (fv.apiKey as string) || "";
              if (!apiKey && !isLocal && ctx.mode === "add") {
                msg.warning("settings.provider.field.api_key.required_first");
                return;
              }
              try {
                let modelList: ModelConfig[];
                if (ctx.mode === "edit" && !apiKey) {
                  modelList = await providerService.listModels(
                    (ctx.entity as Provider)?.id,
                  );
                } else {
                  modelList = await providerService.previewModels(
                    ctx.mode === "edit"
                      ? (ctx.entity as Provider)?.type
                      : ctx.preset
                        ? (ctx.preset as ProviderPreset).slug
                        : "custom",
                    baseUrl,
                    apiKey,
                  );
                }
                modelList = modelList || [];
                setModels(modelList);
                callbacks.setFieldOptions(
                  modelList.map((m) => ({
                    label: m.name || m.id,
                    value: m.id,
                  })),
                );
                msg.success("settings.provider.field.models_fetched", {
                  count: modelList.length,
                });
              } catch (e) {
                msg.error("settings.provider.field.models_fetch_failed", e);
              }
            },
          },
        ],
      },
    ],
    [t, showType, isLocal, isEdit, formatInfo, models, msg],
  );

  const getTitle = () => {
    if (isEdit)
      return t("settings.provider.edit_title", { name: provider.name });
    if (preset)
      return t("settings.provider.connect_title", { name: preset.label });
    return t("settings.provider.add_custom_title", {
      format: translateKey(getProviderFormatLabel(effectiveFormat)),
    });
  };

  const getDescription = () => {
    if (isLocal) return t("settings.provider.description.local");
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
      const input: ProviderInput = {
        id: provider?.id || "",
        name: values.name || (isPreset && preset ? preset.label : ""),
        type: isEdit
          ? provider.type
          : isPreset && preset
            ? preset.slug
            : values.type || "custom",
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
    if (isEdit) {
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
        type: isPreset && preset ? preset.slug : "",
        baseUrl: isPreset && preset ? preset.defaultBaseUrl : "",
        apiKey: "",
        defaultModel: undefined,
      });
      setModels([]);
    }
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
          <Button onClick={onClose}>{t("common.actions.cancel")}</Button>
          <Button
            type="primary"
            loading={saving}
            onClick={() => {
              void handleOk();
            }}
          >
            {isEdit ? t("common.actions.save") : t("settings.provider.connect")}
          </Button>
        </div>
      }
      width="var(--modal-width-form)"
    >
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
                        message: t("settings.provider.field.name.required"),
                      },
                    ]
                  : schema.key === "baseUrl"
                    ? [
                        {
                          required: true,
                          message: t(
                            "settings.provider.field.base_url.required",
                          ),
                        },
                      ]
                    : schema.key === "apiKey" && !isEdit && !isLocal
                      ? [
                          {
                            required: true,
                            message: t(
                              "settings.provider.field.api_key.required",
                            ),
                          },
                        ]
                      : schema.key === "type" && showType
                        ? [
                            {
                              required: true,
                              message: t(
                                "settings.provider.field.type.required",
                              ),
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
                onClick={() => form.setFieldsValue({ defaultModel: m.id })}
              >
                <span className="connect-model-name">{m.name || m.id}</span>
                <span className="connect-model-id">{m.id}</span>
                {m.attachment ? (
                  <span className="connect-model-attachment">
                    {t(
                      `settings.provider.field.attachment_support.${m.attachment.support}`,
                    )}
                  </span>
                ) : null}
              </button>
            ))}
            {models.length > 20 && (
              <Typography.Text
                type="secondary"
                style={{ fontSize: 11, padding: "4px 0" }}
              >
                {t("settings.provider.field.models_more", {
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
