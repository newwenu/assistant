import { Typography } from "antd";
import type React from "react";

interface SettingsSectionProps {
  title: string;
  description?: string;
  extra?: React.ReactNode;
  children: React.ReactNode;
  style?: React.CSSProperties;
}

export const SettingsSection: React.FC<SettingsSectionProps> = ({
  title,
  description,
  extra,
  children,
  style,
}) => (
  <div style={{ marginTop: 24, ...style }}>
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        marginBottom: description ? 4 : 16,
      }}
    >
      <Typography.Title level={5} style={{ margin: 0 }}>
        {title}
      </Typography.Title>
      {extra}
    </div>
    {description && (
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, display: "block", marginBottom: 16 }}
      >
        {description}
      </Typography.Text>
    )}
    {children}
  </div>
);
