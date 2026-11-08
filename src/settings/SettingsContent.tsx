import React from "react";
import { useTranslation } from "react-i18next";
import { Button, Empty, Typography } from "antd";
import { useNavigate, useParams } from "react-router-dom";
import {
  DEFAULT_SETTINGS_PAGE_ID,
  getSettingsPageManifest,
  isSettingsPageId,
} from "@/settings/core/pageRegistry";

const SettingsPageNotFound: React.FC<{ pageId: string }> = ({ pageId }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 48,
      }}
    >
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={
          <>
            <Typography.Text strong>
              {t("settings.not_found.title")}
            </Typography.Text>
            <br />
            <Typography.Text type="secondary">
              {t("settings.not_found.description", { pageId })}
            </Typography.Text>
          </>
        }
      >
        <Button
          type="primary"
          onClick={() => {
            void navigate(`/settings/${DEFAULT_SETTINGS_PAGE_ID}`, {
              replace: true,
            });
          }}
        >
          {t("settings.not_found.back")}
        </Button>
      </Empty>
    </div>
  );
};

export const SettingsContent: React.FC = () => {
  const { t } = useTranslation();
  const { pageId } = useParams<{ pageId: string }>();

  if (!pageId || !isSettingsPageId(pageId)) {
    return <SettingsPageNotFound pageId={pageId ?? ""} />;
  }

  const manifest = getSettingsPageManifest(pageId);
  if (!manifest) return <SettingsPageNotFound pageId={pageId} />;

  const Content = manifest.component;

  return (
    <React.Suspense
      fallback={
        <div style={{ padding: 24, color: "var(--color-text-secondary)" }}>
          {t("common.actions.loading")}
        </div>
      }
    >
      <Content />
    </React.Suspense>
  );
};
