import React, { useState, useEffect, useCallback, type RefObject } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { Provider } from "../../settings/provider/types";
import { onOrbCloseOverlays } from "../../orb/orbOverlays";

interface ProviderSelectorProps {
  providerId: string | null;
  providers: Provider[];
  onProviderChange: (providerId: string) => void;
  width?: number;
  variant?: "default" | "chip";
  openRef?: RefObject<boolean>;
}

export const ProviderSelector: React.FC<ProviderSelectorProps> = ({
  providerId,
  providers,
  onProviderChange,
  width = 140,
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

  useEffect(
    () =>
      onOrbCloseOverlays(() => {
        setOpen(false);
        if (openRef) openRef.current = false;
      }),
    [openRef],
  );

  if (providers.length <= 1) return null;

  if (variant === "chip") {
    return (
      <Select
        value={providerId || undefined}
        onChange={onProviderChange}
        placeholder={t("chat.provider.select_placeholder")}
        className="orb-provider-chip"
        popupMatchSelectWidth={false}
        classNames={{ popup: { root: "orb-dropdown" } }}
        size="small"
        open={open}
        onOpenChange={handleOpenChange}
        options={providers.map((p) => ({
          value: p.id,
          label: p.name,
        }))}
        suffix={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={providerId || undefined}
      onChange={onProviderChange}
      placeholder={t("chat.provider.select_placeholder")}
      style={{ width }}
      size="small"
      options={providers.map((p) => ({
        value: p.id,
        label: p.name,
      }))}
    />
  );
};
