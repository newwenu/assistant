import type React from "react";
import { Button, Divider, Typography } from "antd";
import { ArrowLeftOutlined } from "@ant-design/icons";
import { useNavigate, useParams } from "react-router-dom";
import { translateKey } from "@/i18n";
import {
  getSettingsPageMap,
  isSettingsPageId,
} from "../../core/registry/pageRegistry";

interface SettingsPageShellProps {
  children: React.ReactNode;
  extra?: React.ReactNode;
  wide?: boolean | number;
}

function resolveWideStyle(wide: boolean | number): React.CSSProperties {
  if (wide === true) return { padding: 24 };
  if (wide === false) return { maxWidth: 700, margin: "0 auto", padding: 24 };
  return { maxWidth: wide, margin: "0 auto", padding: 24 };
}

export const SettingsPageShell: React.FC<SettingsPageShellProps> = ({
  children,
  extra,
  wide = false,
}) => {
  const navigate = useNavigate();
  const { pageId } = useParams<{ pageId: string }>();
  const manifest =
    pageId && isSettingsPageId(pageId)
      ? getSettingsPageMap().get(pageId)
      : undefined;
  const title = manifest ? translateKey(manifest.label) : undefined;

  return (
    <div className="settings-page-shell" style={resolveWideStyle(wide)}>
      {manifest && (
        <>
          <div
            className="settings-page-header"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 8,
              marginBottom: 16,
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
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
            {extra}
          </div>
          <Divider style={{ margin: "0 0 16px" }} />
        </>
      )}
      {children}
    </div>
  );
};
