import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  useNavigate,
  useLocation,
  useParams,
} from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { notification } from "antd";
import { Titlebar } from "@/components/Titlebar";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";
import { Layout, Menu } from "antd";
import { HistoryOutlined, SettingOutlined } from "@ant-design/icons";
import {
  WINDOW_MAIN,
  WINDOW_ORB,
  WINDOW_SETTINGS,
  SIDEBAR_WIDTH,
} from "@/config/constants";
import { useOrbStore } from "@/stores/orbStore";
import { useToolStore } from "@/stores/toolStore";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  SETTINGS_PAGES,
  DEFAULT_SETTINGS_PAGE_ID,
  getSettingsPageManifest,
  isSettingsPageId,
} from "@/settings/core/pageRegistry";

import { HistoryManager } from "@/history";
import { ToolBox } from "@/tools/ToolBox";
import { ToolPage } from "@/tools/ToolPage";
import { CommandPalette } from "@/components/commandPalette/CommandPalette";
import { useToolMenuItems } from "@/tools/CollapsibleToolMenu";
import { toolsMap } from "@/tools/registry";

const SettingsContent: React.FC = () => {
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

const MainLayout: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const toolMenuItems = useToolMenuItems(() => {
    void navigate("/");
  });
  const loadFavorites = useToolStore((s) => s.loadFavorites);
  const loadRecentTools = useToolStore((s) => s.loadRecentTools);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);
  const [api, contextHolder] = notification.useNotification();

  const [openKeys, setOpenKeys] = useState<string[]>([
    "toolbox",
    "settings-sub",
  ]);

  useEffect(() => {
    if (location.pathname.startsWith("/tools/")) {
      const toolId = location.pathname.replace("/tools/", "");
      const tool = toolsMap[toolId];
      if (tool) {
        setOpenKeys((prev) => {
          const next = new Set(prev);
          next.add("toolbox");
          next.add(`tool-cat-${tool.category}`);
          return [...next];
        });
      }
    }
    if (location.pathname.startsWith("/settings/")) {
      setOpenKeys((prev) => {
        const next = new Set(prev);
        next.add("settings-sub");
        return [...next];
      });
    }
  }, [location.pathname]);

  useEffect(() => {
    void loadFavorites();
    void loadRecentTools();
    void loadAppConf();
    void loadOrbConf();
    void loadMascotConf();
  }, [
    loadFavorites,
    loadRecentTools,
    loadAppConf,
    loadOrbConf,
    loadMascotConf,
  ]);

  useEffect(() => {
    const unlisten = listen<{ id: string; binding: string; error: string }>(
      "shortcut-warning",
      ({ payload }) => {
        api.warning({
          message: translateKey("common.window.shortcut_register_failed"),
          description: translateKey("common.window.shortcut_occupied", {
            id: payload.id,
            binding: payload.binding,
          }),
          placement: "topRight",
        });
      },
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [api]);

  useEffect(() => {
    const unlisten = listen<string>("navigate", ({ payload }) => {
      void navigate(payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [navigate]);

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
          <Layout.Sider
            width={SIDEBAR_WIDTH}
            style={{
              background: "var(--color-bg-sidebar)",
              borderRight: "1px solid var(--color-border)",
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
            }}
            styles={{
              body: {
                display: "flex",
                flexDirection: "column",
                minHeight: 0,
                overflow: "hidden",
              },
            }}
          >
            <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
              <Menu
                mode="inline"
                selectedKeys={[location.pathname]}
                openKeys={openKeys}
                onOpenChange={(keys) => setOpenKeys(keys)}
                style={{ background: "transparent", border: "none" }}
                items={[
                  ...toolMenuItems,
                  { type: "divider" as const },
                  {
                    key: "/history",
                    label: t("common.navigation.chat_history"),
                    icon: <HistoryOutlined />,
                  },
                  { type: "divider" as const },
                  {
                    key: "settings-sub",
                    label: t("common.navigation.settings"),
                    icon: <SettingOutlined />,
                    children: SETTINGS_PAGES.map((page) => ({
                      key: `/settings/${page.id}`,
                      label: translateKey(page.label),
                    })),
                  },
                ]}
                onClick={({ key }) => {
                  void navigate(key);
                }}
              />
            </div>
          </Layout.Sider>
          <Layout.Content
            style={{
              background: "var(--color-bg)",
              overflow: "auto",
            }}
          >
            <Routes>
              <Route path="/" element={<ToolBox />} />
              <Route path="/tools/:id" element={<ToolPage />} />
              <Route path="/history" element={<HistoryManager />} />
              <Route
                path="/settings"
                element={
                  <Navigate
                    to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`}
                    replace
                  />
                }
              />
              <Route path="/settings/:pageId" element={<SettingsContent />} />
            </Routes>
          </Layout.Content>
        </Layout>
      </Layout>
      <CommandPalette />
    </>
  );
};

const SettingsWindow: React.FC = () => {
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
          <Route path="/settings/:pageId" element={<SettingsContent />} />
        </Routes>
      </Layout.Content>
    </Layout>
  );
};

const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);

  React.useEffect(() => {
    void loadOrbConf();
    void loadMascotConf();
  }, [loadOrbConf, loadMascotConf]);

  const orbClassName = [
    isExpanded ? "orb-expanded" : "",
    `orb-expand-${expandDirection}`,
    `orb-expand-${expandVertical}`,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      data-orb-window
      className={orbClassName}
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
      }}
    >
      <OrbBall />
      <OrbPanel />
    </div>
  );
};

const viewMap: Record<string, React.FC> = {
  [WINDOW_MAIN]: MainLayout,
  [WINDOW_ORB]: OrbWindow,
  [WINDOW_SETTINGS]: SettingsWindow,
};

function App() {
  const webview = getCurrentWebview();
  const View = viewMap[webview.label] ?? MainLayout;
  return (
    <BrowserRouter>
      <View />
    </BrowserRouter>
  );
}

export default App;
