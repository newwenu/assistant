import React, { useState, useEffect, useCallback, type RefObject } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import type { Provider } from "../stores/providerStore";

interface ModelSelectorProps {
  providerId: string | null;
  modelId: string;
  onModelChange: (modelId: string) => void;
  providers: Provider[];
  width?: number;
  variant?: "default" | "chip";
  openRef?: RefObject<boolean>;
}

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  providerId,
  modelId,
  onModelChange,
  providers,
  width = 160,
  variant = "default",
  openRef,
}) => {
  const [open, setOpen] = useState(false);

  const handleOpenChange = useCallback(
    (v: boolean) => {
      setOpen(v);
      if (openRef) openRef.current = v;
    },
    [openRef],
  );

  useEffect(() => {
    const handler = () => {
      setOpen(false);
      if (openRef) openRef.current = false;
    };
    window.addEventListener("orb-close-menus", handler);
    return () => window.removeEventListener("orb-close-menus", handler);
  }, [openRef]);

  const currentProvider = providers.find((p) => p.id === providerId);
  let models: { id: string; name: string }[] = [];
  if (currentProvider) {
    try {
      models = JSON.parse(currentProvider.models || "[]");
    } catch {
      models = [];
    }
  }

  if (variant === "chip") {
    return (
      <Select
        value={modelId || undefined}
        onChange={onModelChange}
        placeholder="模型"
        disabled={!providerId}
        className="orb-model-chip"
        popupMatchSelectWidth={false}
        popupClassName="orb-dropdown"
        size="small"
        open={open}
        onDropdownVisibleChange={handleOpenChange}
        options={models.map((m) => ({
          value: m.id,
          label: m.name || m.id,
        }))}
        suffixIcon={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={modelId || undefined}
      onChange={onModelChange}
      placeholder="选择模型"
      disabled={!providerId}
      style={{ width }}
      size="small"
      options={models.map((m) => ({
        value: m.id,
        label: m.name || m.id,
      }))}
    />
  );
};
