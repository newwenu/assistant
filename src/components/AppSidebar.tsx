import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useLocation } from "react-router-dom";
import { translateKey } from "@/i18n";
import { Button, Layout, Menu } from "antd";
import {
  HistoryOutlined,
  SettingOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import { SIDEBAR_WIDTH, SIDEBAR_COLLAPSED_WIDTH } from "@/config/constants";
import { SETTINGS_DETAIL_PAGES } from "@/settings/core/pageRegistry";
import { useToolMenuItems } from "@/tools/CollapsibleToolMenu";
import { toolsMap } from "@/tools/registry";

export const AppSidebar: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const toolMenuItems = useToolMenuItems(() => {
    void navigate("/");
  });

  const [openKeys, setOpenKeys] = useState<string[]>([
    "toolbox",
    "settings-sub",
  ]);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const titleClickNavRef = useRef(false);

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
      if (titleClickNavRef.current) {
        titleClickNavRef.current = false;
        return;
      }
      setOpenKeys((prev) => {
        const next = new Set(prev);
        next.add("settings-sub");
        return [...next];
      });
    }
  }, [location.pathname]);

  return (
    <Layout.Sider
      width={SIDEBAR_WIDTH}
      collapsedWidth={SIDEBAR_COLLAPSED_WIDTH}
      collapsed={sidebarCollapsed}
      collapsible
      onCollapse={setSidebarCollapsed}
      trigger={null}
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
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: sidebarCollapsed ? "center" : "space-between",
          padding: sidebarCollapsed ? "12px 0" : "12px 16px",
          borderBottom: "1px solid var(--color-border)",
          flexShrink: 0,
          minHeight: 48,
        }}
      >
        {!sidebarCollapsed && (
          <span
            style={{
              fontSize: 15,
              fontWeight: 600,
              color: "var(--color-text-primary)",
              whiteSpace: "nowrap",
              overflow: "hidden",
            }}
          >
            Assistant
          </span>
        )}
        <Button
          type="text"
          size="small"
          onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
          icon={
            sidebarCollapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />
          }
          style={{ color: "var(--color-text-secondary)" }}
        />
      </div>
      <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
        <Menu
          mode="inline"
          inlineCollapsed={sidebarCollapsed}
          selectedKeys={[
            location.pathname === "/"
              ? "toolbox"
              : location.pathname === "/settings/overview"
                ? "settings-sub"
                : location.pathname,
          ]}
          openKeys={sidebarCollapsed ? [] : openKeys}
          onOpenChange={(keys) => setOpenKeys(keys)}
          style={{ background: "transparent", border: "none" }}
          items={[
            ...toolMenuItems,
            { type: "divider" as const },
            {
              key: "/vault",
              label: t("common.navigation.vault"),
              icon: <SafetyCertificateOutlined />,
            },
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
              onTitleClick: () => {
                titleClickNavRef.current = true;
                void navigate(`/settings/overview`);
              },
              children: SETTINGS_DETAIL_PAGES.map((page) => ({
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
  );
};
