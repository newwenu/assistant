import type { TFunction } from "i18next";

import { providerService } from "@/services/providerService";
import { getFormatInfo } from "@/settings/core/provider/providerLookup";
import type { FieldSchema } from "../../core/types/fieldSchema";
import type { ModelConfig, Provider, ProviderPreset } from "../types";
import type { useSettingMessage } from "../../shared/useSettingMessage";

interface BuildFieldSchemasOptions {
  t: TFunction;
  models: ModelConfig[];
  setModels: (models: ModelConfig[]) => void;
  isEdit: boolean;
  isLocal: boolean;
  showType: boolean;
  formatInfo: ReturnType<typeof getFormatInfo>;
  msg: ReturnType<typeof useSettingMessage>;
}

/**
 * 连接弹窗的字段定义：名称/类型/密钥/地址/默认模型，
 * 「获取模型」动作复用同一套预览接口并在成功后就地写入选项。
 */
export function buildFieldSchemas({
  t,
  models,
  setModels,
  isEdit,
  isLocal,
  showType,
  formatInfo,
  msg,
}: BuildFieldSchemasOptions): FieldSchema[] {
  return [
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
              (ctx.preset ? (ctx.preset as ProviderPreset).defaultBaseUrl : "");
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
  ];
}
