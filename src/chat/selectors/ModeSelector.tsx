import React, { useState, useEffect, useCallback } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useAgentModeStore } from "../../stores/agentModeStore";

interface ModeSelectorProps {
  variant?: "default" | "chip";
  onModeChange?: (modeId: string) => void;
}

export const ModeSelector: React.FC<ModeSelectorProps> = ({
  variant = "chip",
  onModeChange,
}) => {
  const [open, setOpen] = useState(false);
  const { t } = useTranslation();
  const modes = useAgentModeStore((s) => s.modes);
  const selectedModeId = useAgentModeStore((s) => s.selectedModeId);
  const selectMode = useAgentModeStore((s) => s.selectMode);
  const loadModes = useAgentModeStore((s) => s.loadModes);

  useEffect(() => {
    if (modes.length === 0) {
      void loadModes();
    }
  }, [modes.length, loadModes]);

  useEffect(() => {
    const handler = () => setOpen(false);
    window.addEventListener("orb-close-menus", handler);
    return () => window.removeEventListener("orb-close-menus", handler);
  }, []);

  const handleChange = useCallback(
    (value: string) => {
      selectMode(value);
      onModeChange?.(value);
    },
    [selectMode, onModeChange],
  );

  const options = modes.map((m) => ({
    value: m.id,
    label: m.name,
  }));

  if (variant === "chip") {
    return (
      <Select
        value={selectedModeId || undefined}
        onChange={handleChange}
        placeholder={t("chat.mode.placeholder")}
        className="orb-mode-chip"
        popupMatchSelectWidth={false}
        classNames={{ popup: { root: "orb-dropdown" } }}
        size="small"
        open={open}
        onOpenChange={setOpen}
        options={options}
        suffix={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={selectedModeId || undefined}
      onChange={handleChange}
      placeholder={t("chat.mode.select_placeholder")}
      style={{ width: 140 }}
      size="small"
      options={options}
    />
  );
};
