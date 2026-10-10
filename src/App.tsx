import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
} from "@/settings/settingsRegistry";
import type { SettingsPageId } from "@/settings/settingsTypes";
import { HistoryManager } from "@/chat/HistoryManager";
import { ToolBox } from "@/tools/ToolBox";
import { ToolPage } from "@/tools/ToolPage";
import { CommandPalette } from "@/components/CommandPalette";
import { useToolMenuItems } from "@/tools/CollapsibleToolMenu";
import { toolsMap } from "@/tools/registry";

const SettingsContent: React.FC = () => {
  const { pageId } = useParams<{ pageId: string }>();

  if (!pageId || !isSettingsPageId(pageId)) {
    return <Navigate to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`} replace />;
  }

  const manifest = getSettingsPageManifest(pageId as SettingsPageId);
  const Content = manifest.component;

  return (
    <React.Suspense
      fallback={
        <div style={{ padding: 24, color: "var(--color-text-secondary)" }}>
          加载中...
        </div>
      }
    >
      <Content />
    </React.Suspense>
  );
};

const MainLayout: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const toolMenuItems = useToolMenuItems(() => navigate("/"));
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
    loadFavorites();
    loadRecentTools();
    loadAppConf();
    loadOrbConf();
    loadMascotConf();
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
          message: "快捷键注册失败",
          description: `${payload.id}（${payload.binding}）可能被其它应用占用`,
          placement: "topRight",
        });
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [api]);

  useEffect(() => {
    const unlisten = listen<string>("navigate", ({ payload }) => {
      navigate(payload);
    });
    return () => {
      unlisten.then((fn) => fn());
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
                    label: "消息历史",
                    icon: <HistoryOutlined />,
                  },
                  { type: "divider" as const },
                  {
                    key: "settings-sub",
                    label: "设置",
                    icon: <SettingOutlined />,
                    children: SETTINGS_PAGES.map((page) => ({
                      key: `/settings/${page.id}`,
                      label: page.label,
                    })),
                  },
                ]}
                onClick={({ key }) => navigate(key)}
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

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);
const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);

const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const setClickThrough = useOrbStore((s) => s.setClickThrough);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);

  React.useEffect(() => {
    if (!IS_MACOS && !IS_WINDOWS) return;
    invoke("set_orb_click_through", { enabled: true });
    setClickThrough(true);
  }, [setClickThrough]);

  React.useEffect(() => {
    loadOrbConf();
    loadMascotConf();
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
