import React from "react";
import {
  getApiFormatByType,
  API_FORMAT_MAP,
  type ApiFormat,
} from "../stores/providerStore";

interface FormatTagProps {
  type: string;
  size?: "small" | "default";
}

const FORMAT_COLORS: Record<ApiFormat, string> = {
  openai: "var(--color-primary)",
  anthropic: "var(--color-format-anthropic)",
  local: "var(--color-success)",
};

export const FormatTag: React.FC<FormatTagProps> = ({
  type,
  size = "default",
}) => {
  const format = getApiFormatByType(type);
  const info = API_FORMAT_MAP.get(format);
  const color = FORMAT_COLORS[format];

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
      {info?.label ?? format}
    </span>
  );
};
