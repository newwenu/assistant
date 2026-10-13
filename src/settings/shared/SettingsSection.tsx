import { Typography } from "antd";
import { translateKey } from "@/i18n";
import type React from "react";
import { getSectionById } from "../core/fieldRegistry";

interface SettingsSectionProps {
  id: string;
  extra?: React.ReactNode;
  children: React.ReactNode;
  style?: React.CSSProperties;
}

export const SettingsSection: React.FC<SettingsSectionProps> = ({
  id,
  extra,
  children,
  style,
}) => {
  const sec = getSectionById(id);
  const title = sec?.title ? translateKey(sec.title) : id;
  const description = sec?.description
    ? translateKey(sec.description)
    : undefined;

  return (
    <div data-section-id={id} style={{ marginTop: 24, ...style }}>
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
          style={{ fontSize: 12, display: "block", marginBottom: 8 }}
        >
          {description}
        </Typography.Text>
      )}
      {children}
    </div>
  );
};
