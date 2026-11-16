import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { translateKey } from "@/i18n";
import { REASONING_EFFORTS, type ReasoningEffort } from "@/chat/core/reasoning";
import { getAllAgentKinds } from "../../core/agentKind/agentModeRegistry";
import "../../core/agentKind/agentModeDefinitions";
import type { FieldSchema } from "../../core/types/fieldSchema";

import { AvatarFieldRender, buildAvatarOptions } from "./avatar";

export interface ModeFormData {
  name: string;
  description: string;
  avatar: string;
  kind: string;
  prompt: string;
  model: string | null;
  temperature: number | null;
  maxTokens: number | null;
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
}

export const DEFAULT_FORM_DATA: ModeFormData = {
  name: "",
  description: "",
  avatar: "",
  kind: "chat",
  prompt: "",
  model: null,
  temperature: null,
  maxTokens: null,
  reasoningEnabled: null,
  reasoningEffort: null,
};

/** 抽屉表单的字段定义：文案与选项全部来自 i18n 与内置头像/推理档位。 */
export function useFieldSchemas(): FieldSchema[] {
  const { t } = useTranslation();

  const kindOptions = useMemo(
    () =>
      getAllAgentKinds().map((k) => ({
        value: k.id,
        label: translateKey(k.label),
      })),
    [],
  );

  const avatarOptions = useMemo(() => buildAvatarOptions(t), [t]);

  const reasoningEffortOptions = useMemo(() => {
    const labels: Record<ReasoningEffort, string> = {
      low: t("settings.agent_mode.field.reasoning_effort.options.low"),
      medium: t("settings.agent_mode.field.reasoning_effort.options.medium"),
      high: t("settings.agent_mode.field.reasoning_effort.options.high"),
    };
    return REASONING_EFFORTS.map((value) => ({
      value,
      label: labels[value],
    }));
  }, [t]);

  return useMemo<FieldSchema[]>(
    () => [
      {
        key: "name",
        type: "string",
        label: t("settings.agent_mode.field.name.label"),
        placeholder: t("settings.agent_mode.field.name.placeholder"),
      },
      {
        key: "description",
        type: "textarea",
        label: t("settings.agent_mode.field.description.label"),
        rows: 2,
        placeholder: t("settings.agent_mode.field.description.placeholder"),
      },
      {
        key: "avatar",
        type: "select",
        label: t("settings.agent_mode.field.avatar.label"),
        description: t("settings.agent_mode.field.avatar.description"),
        options: avatarOptions,
        customRender: AvatarFieldRender,
      },
      {
        key: "kind",
        type: "select",
        label: t("settings.agent_mode.field.kind.label"),
        options: kindOptions,
      },
      {
        key: "prompt",
        type: "textarea",
        label: t("settings.agent_mode.field.prompt.label"),
        rows: 4,
        placeholder: t("settings.agent_mode.field.prompt.placeholder"),
      },
      {
        key: "model",
        type: "select",
        label: t("settings.agent_mode.field.model.label"),
        options: [],
        allowClear: true,
        placeholder: t("settings.agent_mode.field.model.placeholder"),
      },
      {
        key: "temperature",
        type: "slider",
        label: t("settings.agent_mode.field.temperature.label"),
        min: 0,
        max: 2,
        step: 0.1,
        marks: { 0: "0", 0.3: "0.3", 0.7: "0.7", 1: "1", 2: "2" },
      },
      {
        key: "maxTokens",
        type: "number",
        label: t("settings.agent_mode.field.max_tokens.label"),
        min: 256,
        max: 128_000,
        step: 256,
        placeholder: t("settings.agent_mode.field.max_tokens.placeholder"),
      },
      {
        key: "reasoningEnabled",
        type: "boolean",
        label: t("settings.agent_mode.field.reasoning_enabled.label"),
      },
      {
        key: "reasoningEffort",
        type: "select",
        label: t("settings.agent_mode.field.reasoning_effort.label"),
        options: reasoningEffortOptions,
        allowClear: true,
        placeholder: t(
          "settings.agent_mode.field.reasoning_effort.placeholder",
        ),
        visible: (fv) => fv.reasoningEnabled === true,
      },
    ],
    [t, kindOptions, reasoningEffortOptions, avatarOptions],
  );
}
