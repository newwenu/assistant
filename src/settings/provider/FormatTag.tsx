import React from "react";
import { translateKey } from "@/i18n";
import { getApiFormatByType } from "@/stores/providerStore";
import {
  getProviderFormatLabel,
  getProviderFormatColor,
} from "../core/providerFormatRegistry";

interface FormatTagProps {
  type: string;
  size?: "small" | "default";
}

export const FormatTag: React.FC<FormatTagProps> = ({
  type,
  size = "default",
}) => {
  const format = getApiFormatByType(type);
  const labelKey = getProviderFormatLabel(format);
  const color = getProviderFormatColor(format);

  const isSmall = size === "small";

  return (
    <span
      className="format-tag"
      style={{
        color,
        borderColor: color,
        fontSize: isSmall ? 10 : 11,
        padding: isSmall ? "0 4px" : "0 6px",
        lineHeight: isSmall ? "16px" : "18px",
      }}
    >
      {translateKey(labelKey)}
    </span>
  );
};
