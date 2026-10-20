import React from "react";
import { Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { Layout } from "antd";
import { DEFAULT_SETTINGS_PAGE_ID } from "@/settings/core/pageRegistry";
import { Titlebar } from "@/components/Titlebar";
import { AppSidebar } from "@/components/AppSidebar";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { HistoryManager } from "@/history";
import { ToolBox } from "@/tools/ToolBox";
import { ToolPage } from "@/tools/ToolPage";
import { CommandPalette } from "@/components/commandPalette/CommandPalette";
import { SettingsContent } from "@/settings/SettingsContent";
import { useAppInitialization } from "@/hooks/useAppInitialization";
import { useErrorNotifications } from "@/hooks/useErrorNotifications";
import { useNavigationListener } from "@/hooks/useNavigationListener";

export const MainLayout: React.FC = () => {
  const navigate = useNavigate();

  useAppInitialization();
  const { contextHolder } = useErrorNotifications();
  useNavigationListener(navigate);

  return (
    <>
      {contextHolder}
      <Layout
        style={{
          height: "100vh",
          background: "var(--color-bg-layout)",
          overflow: "hidden",
        }}
      >
        <Titlebar />
        <Layout style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
          <AppSidebar />
          <Layout.Content
            style={{
              background: "var(--color-bg)",
              overflow: "auto",
            }}
          >
            <Routes>
              <Route path="/" element={<ToolBox />} />
              <Route path="/tools/:id" element={<ToolPage />} />
              <Route
                path="/history"
                element={
                  <FeatureErrorBoundary featureName="历史记录">
                    <HistoryManager />
                  </FeatureErrorBoundary>
                }
              />
              <Route
                path="/settings"
                element={
                  <Navigate
                    to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`}
                    replace
                  />
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
      </Layout>
      <CommandPalette />
    </>
  );
};
