import React, { useState, useEffect, useCallback } from "react";
import { Select, Tooltip } from "antd";
import type { SelectProps } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useAgentModeStore } from "../../stores/agentModeStore";
import { onOrbCloseOverlays } from "../../orb/orbOverlays";

interface ModeSelectorProps {
  variant?: "default" | "chip";
  /** 当前会话的 `modeId`；为 `null` 时显示占位符。 */
  value: string | null;
  onModeChange?: (modeId: string) => void;
  /** turn 进行中置灰（I38d §4.8）。 */
  disabled?: boolean;
}

interface ModeOption {
  value: string;
  label: string;
}

/**
 * 模式选择的唯一控件：按 `kind` 分组列出全部模式（对话组 / Agent 组），
 * 选中项直接决定链路（`kind === "agent"` 走 agent 布局），无需额外分组切换控件（I38d §4.8）。
 */
export const ModeSelector: React.FC<ModeSelectorProps> = ({
  variant = "chip",
  value,
  onModeChange,
  disabled = false,
}) => {
  const [open, setOpen] = useState(false);
  const { t } = useTranslation();
  const modes = useAgentModeStore((s) => s.modes);
  const selectMode = useAgentModeStore((s) => s.selectMode);
  const loadModes = useAgentModeStore((s) => s.loadModes);

  useEffect(() => {
    if (modes.length === 0) {
      void loadModes();
    }
  }, [modes.length, loadModes]);

  useEffect(() => onOrbCloseOverlays(() => setOpen(false)), []);

  const handleChange = useCallback(
    (next: string) => {
      selectMode(next);
      onModeChange?.(next);
    },
    [selectMode, onModeChange],
  );

  const chatOptions: ModeOption[] = modes
    .filter((m) => m.kind !== "agent")
    .map((m) => ({ value: m.id, label: m.name }));
  const agentOptions: ModeOption[] = modes
    .filter((m) => m.kind === "agent")
    .map((m) => ({ value: m.id, label: m.name }));

  const options: SelectProps["options"] =
    chatOptions.length > 0 && agentOptions.length > 0
      ? [
          { label: t("chat.mode.group_chat"), options: chatOptions },
          { label: t("chat.mode.group_agent"), options: agentOptions },
        ]
      : [...chatOptions, ...agentOptions];

  if (variant === "chip") {
    const select = (
      <Select
        value={value ?? undefined}
        onChange={handleChange}
        placeholder={t("chat.mode.placeholder")}
        className="orb-mode-chip"
        popupMatchSelectWidth={false}
        classNames={{ popup: { root: "orb-dropdown" } }}
        size="small"
        open={disabled ? false : open}
        onOpenChange={setOpen}
        options={options}
        disabled={disabled}
        suffix={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
    return disabled ? (
      <Tooltip title={t("chat.mode.switch_disabled_tooltip")}>
        <span style={{ display: "inline-block", cursor: "not-allowed" }}>
          {select}
        </span>
      </Tooltip>
    ) : (
      select
    );
  }

  return (
    <Select
      value={value ?? undefined}
      onChange={handleChange}
      placeholder={t("chat.mode.select_placeholder")}
      style={{ width: 140 }}
      size="small"
      options={options}
      disabled={disabled}
    />
  );
};
