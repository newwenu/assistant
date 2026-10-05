import React, { useState, useCallback, type RefObject } from "react";
import { Select } from "antd";
import { DownOutlined } from "@ant-design/icons";
import type { Provider } from "../stores/providerStore";

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

  const handleOpenChange = useCallback(
    (v: boolean) => {
      setOpen(v);
      if (openRef) openRef.current = v;
    },
    [openRef],
  );

  if (providers.length <= 1) return null;

  if (variant === "chip") {
    return (
      <Select
        value={providerId || undefined}
        onChange={onProviderChange}
        placeholder="Provider"
        className="orb-provider-chip"
        popupMatchSelectWidth={false}
        popupClassName="orb-dropdown"
        size="small"
        open={open}
        onDropdownVisibleChange={handleOpenChange}
        options={providers.map((p) => ({
          value: p.id,
          label: p.name,
        }))}
        suffixIcon={<DownOutlined style={{ fontSize: 10 }} />}
      />
    );
  }

  return (
    <Select
      value={providerId || undefined}
      onChange={onProviderChange}
      placeholder="选择 Provider"
      style={{ width }}
      size="small"
      options={providers.map((p) => ({
        value: p.id,
        label: p.name,
      }))}
    />
  );
};
