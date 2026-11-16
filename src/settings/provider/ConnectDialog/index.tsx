import React, { useMemo } from "react";
import { Modal, Form, Button, Typography } from "antd";
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
  ApiFormat,
} from "../types";
import { getProviderPreset } from "../../core/provider/providerPresetRegistry";
import { getProviderFormatLabel } from "../../core/provider/providerFormatRegistry";
import type { FieldSchema, FieldContext } from "../../core/types/fieldSchema";
import { SettingField } from "../../shared/renderers/SettingField";
import { FormatTag } from "../FormatTag";
import { useSettingMessage } from "../../shared/useSettingMessage";

import { buildFieldSchemas } from "./fieldSchemas";
import { ModelPreviewList } from "./ModelPreviewList";
import { useConnectForm } from "./useConnectForm";

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
  const msg = useSettingMessage();

  const isEdit = !!provider;
  const effectiveFormat = isEdit
    ? getApiFormatByType(provider.type)
    : (preset?.format ?? customFormat ?? "openai");
  const formatInfo = getFormatInfo(effectiveFormat);
  const isPreset = isEdit ? !!getProviderPreset(provider.type) : !!preset;
  const isLocal = effectiveFormat === "local";
  const showType = !isEdit && !isPreset;

  const { form, saving, models, setModels, handleOk, handleAfterOpen } =
    useConnectForm({ provider, preset, isPreset, onSubmit });

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
    () =>
      buildFieldSchemas({
        t,
        models,
        setModels,
        isEdit,
        isLocal,
        showType,
        formatInfo,
        msg,
      }),
    [t, showType, isLocal, isEdit, formatInfo, models, setModels, msg],
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

        <ModelPreviewList
          models={models}
          onSelect={(id) => form.setFieldsValue({ defaultModel: id })}
        />
      </Form>
    </Modal>
  );
};
