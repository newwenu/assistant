import React from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useParams } from "react-router-dom";
import {
  DEFAULT_SETTINGS_PAGE_ID,
  getSettingsPageManifest,
  isSettingsPageId,
} from "@/settings/core/pageRegistry";

export const SettingsContent: React.FC = () => {
  const { t } = useTranslation();
  const { pageId } = useParams<{ pageId: string }>();

  if (!pageId || !isSettingsPageId(pageId)) {
    return <Navigate to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`} replace />;
  }

  const manifest = getSettingsPageManifest(pageId);
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
