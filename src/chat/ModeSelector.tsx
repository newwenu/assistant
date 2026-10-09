import React, { useState, useEffect, useCallback } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useAgentModeStore } from "../stores/agentModeStore";

interface ModeSelectorProps {
  variant?: "default" | "chip";
  onModeChange?: (modeId: string) => void;
}

export const ModeSelector: React.FC<ModeSelectorProps> = ({
  variant = "chip",
  onModeChange,
}) => {
  const [open, setOpen] = useState(false);
  const modes = useAgentModeStore((s) => s.modes);
  const selectedModeId = useAgentModeStore((s) => s.selectedModeId);
  const selectMode = useAgentModeStore((s) => s.selectMode);
  const loadModes = useAgentModeStore((s) => s.loadModes);

  useEffect(() => {
    if (modes.length === 0) {
      loadModes();
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
        placeholder="模式"
        className="orb-mode-chip"
        popupMatchSelectWidth={false}
        popupClassName="orb-dropdown"
        size="small"
        open={open}
        onDropdownVisibleChange={setOpen}
        options={options}
        suffixIcon={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={selectedModeId || undefined}
      onChange={handleChange}
      placeholder="选择模式"
      style={{ width: 140 }}
      size="small"
      options={options}
    />
  );
};
