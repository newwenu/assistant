import React from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { Layout } from "antd";
import { DEFAULT_SETTINGS_PAGE_ID } from "@/settings/core/pageRegistry";
import { Titlebar } from "@/components/Titlebar";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { SettingsContent } from "@/settings/SettingsContent";

export const SettingsWindow: React.FC = () => {
  return (
    <Layout
      style={{
        height: "100vh",
        background: "var(--color-bg)",
        overflow: "hidden",
      }}
    >
      <Titlebar title="Settings" />
      <Layout.Content
        style={{
          background: "var(--color-bg)",
          overflow: "auto",
          flex: 1,
          minHeight: 0,
        }}
      >
        <Routes>
          <Route
            path="/settings"
            element={
              <Navigate to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`} replace />
            }
          />
          <Route
            path="/settings/:pageId"
            element={
              <FeatureErrorBoundary featureName="设置">
                <SettingsContent />
              </FeatureErrorBoundary>
            }
          />
        </Routes>
      </Layout.Content>
    </Layout>
  );
};
