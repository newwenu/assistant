import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useLocation } from "react-router-dom";
import { translateKey } from "@/i18n";
import { Button, Layout, Menu } from "antd";
import type { MenuProps } from "antd";
import {
  HistoryOutlined,
  SettingOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
} from "@ant-design/icons";
import { SIDEBAR_WIDTH, SIDEBAR_COLLAPSED_WIDTH } from "@/config/constants";
import {
  getSettingsDetailPages,
  getSettingsPages,
} from "@/settings/core/registry/pageRegistry";
import {
  favoriteMenuKey,
  resolveMenuKey,
  useToolMenuItems,
} from "@/tools/ui/CollapsibleToolMenu";
import { toolsMap } from "@/tools/framework/registry";
import { useToolStore } from "@/stores/toolStore";
import { getFeatures } from "@/features/registry";
import { resolveSidebarKeys, resolveSidebarTarget } from "@/features/nav";
import type { PreparedSidebarEntry } from "@/features/aggregate";
import type { SidebarSlot } from "@/features/types";

type SidebarItem = NonNullable<MenuProps["items"]>[number];

function toFeatureMenuItem(entry: PreparedSidebarEntry): SidebarItem {
  return {
    key: entry.key,
    label: translateKey(entry.label),
    icon: React.createElement(entry.icon),
  };
}

function nextOpenKeys(
  prev: string[],
  pathname: string,
  favoriteToolIds: string[],
): string[] {
  const next = new Set(prev);
  let changed = false;

  if (pathname.startsWith("/tools/")) {
    const toolId = pathname.replace("/tools/", "");
    const tool = toolsMap[toolId];
    if (tool) {
      const keys = ["toolbox", `tool-cat-${tool.category}`];
      if (favoriteToolIds.includes(toolId)) keys.push("tool-favorites");
      for (const key of keys) {
        if (!next.has(key)) {
          next.add(key);
          changed = true;
        }
      }
    }
  }

  if (pathname.startsWith("/settings/") && !next.has("settings-sub")) {
    next.add("settings-sub");
    changed = true;
  }

  return changed ? [...next] : prev;
}

export const AppSidebar: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);
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
    if (
      location.pathname.startsWith("/settings/") &&
      titleClickNavRef.current
    ) {
      titleClickNavRef.current = false;
      return;
    }
    setOpenKeys((prev) =>
      nextOpenKeys(prev, location.pathname, favoriteToolIds),
    );
  }, [location.pathname, favoriteToolIds]);

  const selectedKeys = useMemo(() => {
    if (location.pathname === "/") return ["toolbox"];
    if (location.pathname === "/settings/overview") return ["settings-sub"];
    if (location.pathname.startsWith("/tools/")) {
      const toolId = location.pathname.replace("/tools/", "");
      if (favoriteToolIds.includes(toolId)) {
        return [location.pathname, favoriteMenuKey(toolId)];
      }
      return [location.pathname];
    }
    return resolveSidebarKeys(
      location.pathname,
      getFeatures().routes,
      getSettingsPages(),
    );
  }, [location.pathname, favoriteToolIds]);

  const menuItems = useMemo<NonNullable<MenuProps["items"]>>(() => {
    const features = getFeatures();
    const bySlot = (slot: SidebarSlot) =>
      features.sidebar
        .filter((entry) => entry.slot === slot)
        .map(toFeatureMenuItem);

    return [
      ...toolMenuItems,
      { type: "divider" as const },
      ...bySlot("after-tools"),
      ...bySlot("before-history"),
      {
        key: "/history",
        label: t("common.navigation.chat_history"),
        icon: <HistoryOutlined />,
      },
      ...bySlot("after-history"),
      ...bySlot("before-settings"),
      { type: "divider" as const },
      {
        key: "settings-sub",
        label: t("common.navigation.settings"),
        icon: <SettingOutlined />,
        onTitleClick: () => {
          titleClickNavRef.current = true;
          void navigate(`/settings/overview`);
        },
        children: getSettingsDetailPages().map((page) => ({
          key: `/settings/${page.id}`,
          label: translateKey(page.label),
        })),
      },
    ];
  }, [toolMenuItems, t, navigate]);

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
          selectedKeys={selectedKeys}
          openKeys={sidebarCollapsed ? [] : openKeys}
          onOpenChange={(keys) => setOpenKeys(keys)}
          style={{ background: "transparent", border: "none" }}
          items={menuItems}
          onClick={({ key }) => {
            const features = getFeatures();
            const target = features.sidebar.some((entry) => entry.key === key)
              ? resolveSidebarTarget(key, features.routes, getSettingsPages())
              : resolveMenuKey(key);
            void navigate(target);
          }}
        />
      </div>
    </Layout.Sider>
  );
};
