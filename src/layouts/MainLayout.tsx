import React, { useEffect } from "react";
import {
  Routes,
  Route,
  Navigate,
  useLocation,
  useNavigate,
} from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { DEFAULT_SETTINGS_PAGE_ID } from "@/settings/core/pageRegistry";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { HistoryManager } from "@/history";
import { ToolBox } from "@/tools/ToolBox";
import { ToolPage } from "@/tools/ToolPage";
import { resolveNavigateMode } from "@/tools/miniNavigate";
import { CommandPalette } from "@/components/commandPalette/CommandPalette";
import { SettingsContent } from "@/settings/SettingsContent";
import { useAppInitialization } from "@/hooks/useAppInitialization";
import { useErrorNotifications } from "@/hooks/useErrorNotifications";
import { useNavigationListener } from "@/hooks/useNavigationListener";
import { getFeatures } from "@/features/registry";
import { UiEvent } from "@/types/events";
import { useWindowModeStore } from "@/stores/windowModeStore";
import { notifyInfo } from "@/stores/notifyStore";
import { AppShell } from "./AppShell";

export const MainLayout: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const mode = useWindowModeStore((s) => s.mode);
  const setMode = useWindowModeStore((s) => s.setMode);
  const applyExternalMode = useWindowModeStore((s) => s.applyExternalMode);

  useAppInitialization();
  const { contextHolder } = useErrorNotifications();
  useNavigationListener(navigate);

  useEffect(() => {
    const unlisten = listen<{ mode: "normal" | "mini" }>(
      UiEvent.WindowModeChanged,
      ({ payload }) => {
        applyExternalMode(payload.mode);
      },
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [applyExternalMode]);

  const handleToggleMode = () => {
    void (async () => {
      if (mode === "mini") {
        await setMode("normal");
        return;
      }
      if (resolveNavigateMode(location.pathname) !== "mini") {
        notifyInfo("mini.page_unsupported", "mini.page_unsupported_desc");
        return;
      }
      await setMode("mini");
    })();
  };

  return (
    <>
      <AppShell
        mode={mode}
        onToggleMode={handleToggleMode}
        contextHolder={contextHolder}
      >
        <Routes>
          <Route path="/" element={<ToolBox />} />
          <Route path="/tools/:id" element={<ToolPage />} />
          {getFeatures().routes.map((route) => (
            <Route
              key={route.path}
              path={route.path}
              element={
                <FeatureErrorBoundary featureName={route.boundary}>
                  <route.element />
                </FeatureErrorBoundary>
              }
            />
          ))}
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
      </AppShell>
      <CommandPalette />
    </>
  );
};
