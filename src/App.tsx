import React, { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  BrowserRouter,
  Routes,
  Route,
  useNavigate,
  useLocation,
} from "react-router-dom";
import { Titlebar } from "@/components/Titlebar";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";
import { Layout, Typography, Menu } from "antd";
import {
  WINDOW_MAIN,
  WINDOW_ORB,
  WINDOW_SETTINGS,
  SIDEBAR_WIDTH,
} from "@/config/constants";
import { useOrbStore } from "@/stores/orbStore";
import { useToolStore } from "@/stores/toolStore";
import { ProviderManager } from "@/settings/ProviderManager";
import { TemplateManager } from "@/settings/TemplateManager";
import { HistoryManager } from "@/chat/HistoryManager";
import { ToolBox } from "@/tools/ToolBox";
import { ToolPage } from "@/tools/ToolPage";
import { CommandPalette } from "@/tools/CommandPalette";
import { toolsByCategory, toolsMap } from "@/tools/registry";

const SettingsPage: React.FC = () => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        color: "var(--color-text-tertiary)",
      }}
    >
      <Typography.Text type="secondary">设置</Typography.Text>
    </div>
  );
};

const MainLayout: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const loadFavorites = useToolStore((s) => s.loadFavorites);
  const loadRecentTools = useToolStore((s) => s.loadRecentTools);

  useEffect(() => {
    loadFavorites();
    loadRecentTools();
  }, [loadFavorites, loadRecentTools]);

  const isToolRoute = location.pathname.startsWith("/tools/");

  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);

  const buildToolCategoryItems = () => {
    const items: {
      key: string;
      label: string;
      children?: { key: string; label: string }[];
    }[] = [];

    if (favoriteToolIds.length > 0) {
      items.push({
        key: "fav-group",
        label: "★ 收藏",
        children: favoriteToolIds
          .map((id) => toolsMap[id])
          .filter(Boolean)
          .map((t) => ({ key: `/tools/${t.id}`, label: t.name })),
      });
    }

    for (const group of toolsByCategory) {
      items.push({
        key: `cat-${group.category}`,
        label: `${group.meta.label}`,
        children: group.tools.map((t) => ({
          key: `/tools/${t.id}`,
          label: t.name,
        })),
      });
    }

    return items;
  };

  return (
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
        >
          <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
            <Menu
              mode="inline"
              selectedKeys={[isToolRoute ? "/" : location.pathname]}
              defaultOpenKeys={["tool-group"]}
              style={{ background: "transparent", border: "none" }}
              items={[
                {
                  key: "tool-group",
                  label: "工具箱",
                  type: "group" as const,
                  children: [
                    { key: "/", label: "全部工具" },
                    ...buildToolCategoryItems(),
                  ],
                },
                { type: "divider" as const },
                { key: "/history", label: "消息历史" },
                { type: "divider" as const },
                {
                  key: "settings-group",
                  label: "设置",
                  type: "group" as const,
                  children: [
                    { key: "/settings", label: "通用设置" },
                    { key: "/settings/providers", label: "Provider 管理" },
                    { key: "/settings/templates", label: "Prompt 模板" },
                  ],
                },
              ]}
              onClick={({ key }) => navigate(key)}
            />
          </div>
          <CommandPalette />
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
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/settings/providers" element={<ProviderManager />} />
            <Route path="/settings/templates" element={<TemplateManager />} />
          </Routes>
        </Layout.Content>
      </Layout>
    </Layout>
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
          <Route path="/settings" element={<SettingsPage />} />
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

  React.useEffect(() => {
    if (!IS_MACOS && !IS_WINDOWS) return;
    invoke("set_orb_click_through", { enabled: true });
    setClickThrough(true);
  }, [setClickThrough]);

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
