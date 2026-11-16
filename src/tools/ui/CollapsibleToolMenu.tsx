import { useMemo } from "react";
import { HeartFilled, AppstoreOutlined } from "@ant-design/icons";
import type { MenuProps } from "antd";
import { translateKey } from "@/i18n";
import { useToolStore } from "@/stores/toolStore";
import {
  toolsByCategory as allToolsByCategory,
  toolsMap,
} from "../framework/registry";
import { CATEGORY_MAP } from "../framework/categories";

type MenuItem = NonNullable<MenuProps["items"]>[number];

const FAVORITE_MENU_KEY_PREFIX = "favorite:";

export function favoriteMenuKey(toolId: string): string {
  return `${FAVORITE_MENU_KEY_PREFIX}/tools/${toolId}`;
}

export function resolveMenuKey(key: string): string {
  return key.startsWith(FAVORITE_MENU_KEY_PREFIX)
    ? key.slice(FAVORITE_MENU_KEY_PREFIX.length)
    : key;
}

export function useToolMenuItems(onToolboxTitleClick?: () => void): MenuItem[] {
  const favoriteToolIds = useToolStore((s) => s.favoriteToolIds);

  return useMemo(() => {
    const children: MenuItem[] = [];

    const favoriteTools = favoriteToolIds
      .map((id) => toolsMap[id])
      .filter(Boolean);

    if (favoriteTools.length > 0) {
      children.push({
        key: "tool-favorites",
        icon: <HeartFilled style={{ color: "var(--color-error)" }} />,
        label: translateKey("tools.favorites"),
        children: favoriteTools.map((t) => ({
          key: favoriteMenuKey(t.id),
          label: translateKey(t.name),
        })),
      });
    }

    for (const group of allToolsByCategory) {
      const CategoryIcon = CATEGORY_MAP[group.category]?.icon;
      children.push({
        key: `tool-cat-${group.category}`,
        icon: CategoryIcon ? <CategoryIcon /> : undefined,
        label: translateKey(group.meta.label),
        children: group.tools.map((t) => ({
          key: `/tools/${t.id}`,
          label: translateKey(t.name),
        })),
      });
    }

    return [
      {
        key: "toolbox",
        icon: <AppstoreOutlined />,
        label: translateKey("tools.toolbox"),
        onTitleClick: onToolboxTitleClick
          ? () => onToolboxTitleClick()
          : undefined,
        children,
      },
    ];
  }, [favoriteToolIds, onToolboxTitleClick]);
}
