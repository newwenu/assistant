import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Titlebar } from "@/components/Titlebar";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";
import { Layout, Typography } from "antd";
import {
  WINDOW_MAIN,
  WINDOW_ORB,
  WINDOW_SETTINGS,
  SIDEBAR_WIDTH,
} from "@/config/constants";
import { useOrbStore } from "@/stores/orbStore";

const ToolboxPage: React.FC = () => {
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
      <Typography.Text type="secondary">选择左侧工具开始使用</Typography.Text>
    </div>
  );
};

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
  return (
    <BrowserRouter>
      <Layout style={{ height: "100vh", background: "var(--color-bg-layout)" }}>
        <Titlebar />
        <Layout>
          <Layout.Sider
            width={SIDEBAR_WIDTH}
            style={{
              background: "var(--color-bg-sidebar)",
              borderRight: "1px solid var(--color-border)",
            }}
          >
            <div style={{ padding: "16px 16px 8px" }}>
              <Typography.Text
                strong
                style={{ fontSize: 15, color: "var(--color-text)" }}
              >
                工具箱
              </Typography.Text>
            </div>
          </Layout.Sider>
          <Layout.Content
            style={{
              background: "var(--color-bg)",
              overflow: "auto",
            }}
          >
            <Routes>
              <Route path="/" element={<ToolboxPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </Layout.Content>
        </Layout>
      </Layout>
    </BrowserRouter>
  );
};

const SettingsWindow: React.FC = () => {
  return (
    <BrowserRouter>
      <Layout style={{ height: "100vh", background: "var(--color-bg)" }}>
        <Titlebar title="Settings" />
        <Layout.Content
          style={{ background: "var(--color-bg)", overflow: "auto" }}
        >
          <Routes>
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </Layout.Content>
      </Layout>
    </BrowserRouter>
  );
};

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);

const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);

  React.useEffect(() => {
    if (!IS_MACOS) return;
    invoke("set_orb_click_through", { enabled: true });
  }, []);

  return (
    <div
      data-orb-window
      className={isExpanded ? "orb-expanded" : undefined}
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
  return <View />;
}

export default App;
