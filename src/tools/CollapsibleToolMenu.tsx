import React, { useState, useEffect } from "react";
import { Menu, Typography } from "antd";
import { HeartFilled, RightOutlined, DownOutlined } from "@ant-design/icons";
import { useNavigate, useLocation } from "react-router-dom";
import { useToolStore } from "@/stores/toolStore";
import { toolsMap } from "./registry";
import type { ToolDefinition } from "./types";

const COLLAPSED_KEY = "tool-menu-collapsed";

function getCollapsedState(): Record<string, boolean> {
  try {
    const stored = localStorage.getItem(COLLAPSED_KEY);
    return stored ? JSON.parse(stored) : {};
  } catch {
    return {};
  }
}

export const CollapsibleToolMenu: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);
  const toolsByCategory = useToolStore((s) => s.toolsByCategory);
  const [collapsed, setCollapsed] =
    useState<Record<string, boolean>>(getCollapsedState);

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
  }, [collapsed]);

  const toggleCollapse = (key: string) => {
    setCollapsed((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const favoriteTools = favoriteToolIds
    .map((id) => toolsMap[id])
    .filter(Boolean) as ToolDefinition[];

  const selectedKey = location.pathname.startsWith("/tools/")
    ? location.pathname
    : location.pathname;

  return (
    <div>
      {favoriteTools.length > 0 && (
        <div style={{ marginBottom: 4 }}>
          <div
            onClick={() => toggleCollapse("__favorites__")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "8px 16px 4px",
              cursor: "pointer",
              userSelect: "none",
            }}
          >
            {collapsed["__favorites__"] ? (
              <RightOutlined style={{ fontSize: 10, opacity: 0.5 }} />
            ) : (
              <DownOutlined style={{ fontSize: 10, opacity: 0.5 }} />
            )}
            <HeartFilled style={{ fontSize: 12, color: "#ff4d4f" }} />
            <Typography.Text
              strong
              style={{ fontSize: 13, color: "var(--color-text-secondary)" }}
            >
              收藏
            </Typography.Text>
          </div>
          {!collapsed["__favorites__"] && (
            <Menu
              mode="inline"
              selectedKeys={[selectedKey]}
              style={{ background: "transparent", border: "none" }}
              items={favoriteTools.map((t) => ({
                key: `/tools/${t.id}`,
                label: t.name,
              }))}
              onClick={({ key }) => navigate(key)}
            />
          )}
        </div>
      )}

      {toolsByCategory.map((group) => (
        <div key={group.category} style={{ marginBottom: 4 }}>
          <div
            onClick={() => toggleCollapse(group.category)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "8px 16px 4px",
              cursor: "pointer",
              userSelect: "none",
            }}
          >
            {collapsed[group.category] ? (
              <RightOutlined style={{ fontSize: 10, opacity: 0.5 }} />
            ) : (
              <DownOutlined style={{ fontSize: 10, opacity: 0.5 }} />
            )}
            <Typography.Text
              strong
              style={{ fontSize: 13, color: "var(--color-text-secondary)" }}
            >
              {group.meta.label}
            </Typography.Text>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              ({group.tools.length})
            </Typography.Text>
          </div>
          {!collapsed[group.category] && (
            <Menu
              mode="inline"
              selectedKeys={[selectedKey]}
              style={{ background: "transparent", border: "none" }}
              items={group.tools.map((t) => ({
                key: `/tools/${t.id}`,
                label: t.name,
              }))}
              onClick={({ key }) => navigate(key)}
            />
          )}
        </div>
      ))}
    </div>
  );
};
