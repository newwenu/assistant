import React, { useState, useEffect, useCallback, useMemo } from "react";
import { Select, Tooltip } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "../../i18n";
import { useChatStore } from "../../stores/chatStore";
import { useProviderStore } from "../../stores/providerStore";
import { useAgentModeStore } from "../../stores/agentModeStore";
import {
  reasoningChoices,
  resolveReasoningChoice,
  unsupportedEffortSummary,
  REASONING_DEFAULT,
  REASONING_INHERIT,
  REASONING_OFF,
  REASONING_ON,
  type ReasoningChoice,
} from "../core/reasoning";

interface ReasoningSelectorProps {
  variant?: "default" | "chip";
}

const EFFORT_LABEL_KEYS: Record<string, string> = {
  minimal: "chat.reasoning_depth.minimal",
  low: "chat.reasoning_depth.low",
  medium: "chat.reasoning_depth.medium",
  high: "chat.reasoning_depth.high",
  xhigh: "chat.reasoning_depth.xhigh",
  max: "chat.reasoning_depth.max",
};

export const ReasoningSelector: React.FC<ReasoningSelectorProps> = ({
  variant = "chip",
}) => {
  const [open, setOpen] = useState(false);
  const { t } = useTranslation();
  const reasoningChoice = useChatStore((s) => s.reasoningChoice);
  const setReasoningChoice = useChatStore((s) => s.setReasoningChoice);
  const selectedProviderId = useChatStore((s) => s.selectedProviderId);
  const selectedModelId = useChatStore((s) => s.selectedModelId);
  const providers = useProviderStore((s) => s.providers);
  const modes = useAgentModeStore((s) => s.modes);
  const selectedModeId = useAgentModeStore((s) => s.selectedModeId);

  useEffect(() => {
    const handler = () => setOpen(false);
    window.addEventListener("orb-close-menus", handler);
    return () => window.removeEventListener("orb-close-menus", handler);
  }, []);

  const reasoningLevels = useMemo(() => {
    const provider = providers.find((p) => p.id === selectedProviderId);
    return provider?.models.find((m) => m.id === selectedModelId)
      ?.reasoningLevels;
  }, [providers, selectedProviderId, selectedModelId]);

  /** 该端点会丢弃的档位；`providers` 未加载时为空（不过滤，保持现行为） */
  const unsupportedEfforts = useMemo(() => {
    const provider = providers.find((p) => p.id === selectedProviderId);
    return provider?.effortSupport.find((s) => s.modelId === selectedModelId)
      ?.unsupported;
  }, [providers, selectedProviderId, selectedModelId]);

  const choices = useMemo(
    () => reasoningChoices(reasoningLevels ?? undefined, unsupportedEfforts),
    [reasoningLevels, unsupportedEfforts],
  );

  const mode = modes.find((m) => m.id === selectedModeId);

  const resolved = resolveReasoningChoice(reasoningChoice, mode);
  const displayChoice = choices.includes(resolved)
    ? resolved
    : REASONING_DEFAULT;

  useEffect(() => {
    if (
      reasoningChoice !== REASONING_INHERIT &&
      !choices.includes(reasoningChoice)
    ) {
      setReasoningChoice(REASONING_INHERIT);
    }
  }, [choices, reasoningChoice, setReasoningChoice]);

  const labelFor = (value: ReasoningChoice): string => {
    if (value === REASONING_DEFAULT) return t("chat.reasoning_depth.default");
    if (value === REASONING_OFF) return t("chat.reasoning_depth.off");
    if (value === REASONING_ON) return t("chat.reasoning_depth.on");
    const key = EFFORT_LABEL_KEYS[value];
    return key ? translateKey(key, { defaultValue: value }) : value;
  };

  const options = choices.map((value) => ({
    value,
    label: labelFor(value),
  }));

  const baseTooltip =
    reasoningChoice === REASONING_INHERIT
      ? t("chat.reasoning_depth.from_mode", { value: labelFor(displayChoice) })
      : t("chat.reasoning_depth.overridden");

  const unsupportedSummary = unsupportedEffortSummary(
    unsupportedEfforts,
    labelFor,
  );
  const unsupportedNote = unsupportedSummary
    ? t("chat.reasoning_depth.unsupported", { value: unsupportedSummary })
    : null;

  const tooltip = unsupportedNote ? (
    <div>
      <div>{baseTooltip}</div>
      <div>{unsupportedNote}</div>
    </div>
  ) : (
    baseTooltip
  );

  const handleChange = useCallback(
    (value?: ReasoningChoice) => {
      setReasoningChoice(value ?? REASONING_INHERIT);
      setOpen(false);
    },
    [setReasoningChoice],
  );

  const select = (
    <Select<ReasoningChoice>
      value={displayChoice}
      onChange={handleChange}
      className={variant === "chip" ? "orb-reasoning-chip" : undefined}
      popupMatchSelectWidth={false}
      classNames={{ popup: { root: "orb-dropdown" } }}
      size="small"
      style={variant === "default" ? { width: 140 } : undefined}
      open={open}
      onOpenChange={setOpen}
      options={options}
      suffix={<DownOutlined style={{ fontSize: 10 }} />}
      aria-label={t("chat.reasoning_depth.label")}
    />
  );

  return (
    <Tooltip title={tooltip} placement="top">
      {select}
    </Tooltip>
  );
};
