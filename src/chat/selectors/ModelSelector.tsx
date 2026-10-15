import React, { useState, useEffect, useCallback, type RefObject } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { Provider } from "../../settings/provider/types";

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
  const { t } = useTranslation();

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
      models = JSON.parse(currentProvider.models || "[]") as {
        id: string;
        name: string;
      }[];
    } catch {
      models = [];
    }
  }

  if (variant === "chip") {
    return (
      <Select
        value={modelId || undefined}
        onChange={onModelChange}
        placeholder={t("chat.model.placeholder")}
        disabled={!providerId}
        className="orb-model-chip"
        popupMatchSelectWidth={false}
        classNames={{ popup: { root: "orb-dropdown" } }}
        size="small"
        open={open}
        onOpenChange={handleOpenChange}
        options={models.map((m) => ({
          value: m.id,
          label: m.name || m.id,
        }))}
        suffix={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={modelId || undefined}
      onChange={onModelChange}
      placeholder={t("chat.model.select_placeholder")}
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
