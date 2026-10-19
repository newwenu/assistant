import type React from "react";
import { Button, Divider, Typography } from "antd";
import { ArrowLeftOutlined } from "@ant-design/icons";
import { useNavigate, useParams } from "react-router-dom";
import { translateKey } from "@/i18n";
import { SETTINGS_PAGE_MAP, isSettingsPageId } from "../../core/pageRegistry";

interface SettingsPageShellProps {
  children: React.ReactNode;
}

export const SettingsPageShell: React.FC<SettingsPageShellProps> = ({
  children,
}) => {
  const navigate = useNavigate();
  const { pageId } = useParams<{ pageId: string }>();
  const manifest =
    pageId && isSettingsPageId(pageId)
      ? SETTINGS_PAGE_MAP.get(pageId)
      : undefined;
  const title = manifest ? translateKey(manifest.label) : undefined;

  return (
    <div
      className="settings-page-shell"
      style={{ maxWidth: 700, margin: "0 auto", padding: 24 }}
    >
      {manifest && (
        <>
          <div
            className="settings-page-header"
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              marginBottom: 16,
            }}
          >
            <Button
              type="text"
              size="small"
              icon={<ArrowLeftOutlined />}
              onClick={() => {
                void navigate("/settings/overview");
              }}
              style={{ color: "var(--color-text-secondary)" }}
            />
            <Typography.Title level={4} style={{ margin: 0 }}>
              {title}
            </Typography.Title>
          </div>
          <Divider style={{ margin: "0 0 16px" }} />
        </>
      )}
      {children}
    </div>
  );
};
