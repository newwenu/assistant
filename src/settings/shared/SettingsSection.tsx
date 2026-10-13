import { Typography } from "antd";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const sec = getSectionById(id);
  const title = sec?.title ? t(sec.title) : id;
  const description = sec?.description ? t(sec.description) : undefined;

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
