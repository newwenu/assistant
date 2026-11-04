import { Layout } from "antd";
import type { ReactNode } from "react";
import type { WindowMode } from "@/stores/windowModeStore";
import { NormalLayout } from "./NormalLayout";
import { MiniLayout } from "./MiniLayout";

interface AppShellProps {
  mode: WindowMode;
  onToggleMode: () => void;
  contextHolder?: ReactNode;
  children: ReactNode;
}

export const AppShell = ({
  mode,
  onToggleMode,
  contextHolder,
  children,
}: AppShellProps) => {
  const layout = mode === "mini" ? MiniLayout : NormalLayout;
  const LayoutTitlebar = layout.Titlebar;
  const LayoutSidebar = layout.Sidebar;

  return (
    <>
      {contextHolder}
      <Layout
        data-window-mode={mode}
        style={{
          height: "100vh",
          background: "var(--color-bg-layout)",
          overflow: "hidden",
        }}
      >
        <LayoutTitlebar onToggleMode={onToggleMode} />
        <Layout style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
          {LayoutSidebar && <LayoutSidebar />}
          {/**
           * 内容宿主保持稳定：切换 normal / mini 时 Routes 子树不重挂载，
           * 工具页输入状态不会丢。两套外壳只替换标题栏与侧栏。
           */}
          <Layout.Content
            style={{
              background: "var(--color-bg)",
              overflow: "auto",
            }}
          >
            {children}
          </Layout.Content>
        </Layout>
      </Layout>
    </>
  );
};
