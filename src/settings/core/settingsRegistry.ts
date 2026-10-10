import React from "react";
import type { ComponentType } from "react";
import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./settingsTypes";

type SettingsPageInput = Omit<SettingsPageManifest, "component"> & {
  component: React.LazyExoticComponent<ComponentType>;
};

export function definePage(page: SettingsPageInput): SettingsPageManifest {
  return { ...page };
}

const general = definePage({
  id: "general",
  categoryId: "general",
  label: "通用设置",
  keywords: ["language", "launch", "tray", "top", "startup"],
  component: React.lazy(() =>
    import("../pages/GeneralSettings").then((m) => ({ default: m.GeneralSettings })),
  ),
});

const appearance = definePage({
  id: "appearance",
  categoryId: "general",
  label: "外观设置",
  keywords: ["theme", "dark", "light", "orb", "ball", "panel"],
  component: React.lazy(() =>
    import("../pages/AppearanceSettings").then((m) => ({
      default: m.AppearanceSettings,
    })),
  ),
});

const shortcuts = definePage({
  id: "shortcuts",
  categoryId: "general",
  label: "快捷键",
  keywords: ["keyboard", "shortcut", "keybinding", "hotkey"],
  component: React.lazy(() =>
    import("../pages/ShortcutSettings").then((m) => ({ default: m.ShortcutSettings })),
  ),
});

const providers = definePage({
  id: "providers",
  categoryId: "provider",
  label: "Provider 管理",
  keywords: ["provider", "api", "model", "openai", "preset"],
  component: React.lazy(() =>
    import("../provider/ProviderManager").then((m) => ({ default: m.ProviderManager })),
  ),
});

const agents = definePage({
  id: "agents",
  categoryId: "provider",
  label: "Agent 模式",
  keywords: ["agent", "mode", "kind", "quick", "deep"],
  component: React.lazy(() =>
    import("../pages/AgentModeManager").then((m) => ({ default: m.AgentModeManager })),
  ),
});

const about = definePage({
  id: "about",
  categoryId: "about",
  label: "关于",
  keywords: ["version", "rustc", "build", "about"],
  component: React.lazy(() =>
    import("../pages/AboutPage").then((m) => ({ default: m.AboutPage })),
  ),
});

export const SETTINGS_PAGES: readonly SettingsPageManifest[] = [
  general,
  appearance,
  shortcuts,
  providers,
  agents,
  about,
];

export const SETTINGS_PAGE_MAP = new Map<SettingsPageId, SettingsPageManifest>(
  SETTINGS_PAGES.map((p) => [p.id, p]),
);

export interface SettingsCategory {
  id: SettingsCategoryId;
  label: string;
  pages: readonly SettingsPageManifest[];
}

const CATEGORY_ORDER: readonly SettingsCategoryId[] = [
  "general",
  "provider",
  "about",
];

const CATEGORY_LABELS: Record<SettingsCategoryId, string> = {
  general: "通用",
  provider: "服务",
  about: "关于",
};

export const SETTINGS_CATEGORIES: readonly SettingsCategory[] =
  CATEGORY_ORDER.map((id) => ({
    id,
    label: CATEGORY_LABELS[id],
    pages: SETTINGS_PAGES.filter((p) => p.categoryId === id),
  }));

export const DEFAULT_SETTINGS_PAGE_ID: SettingsPageId = "general";

export function getSettingsPageManifest(
  pageId: SettingsPageId,
): SettingsPageManifest {
  return (
    SETTINGS_PAGE_MAP.get(pageId) ??
    SETTINGS_PAGE_MAP.get(DEFAULT_SETTINGS_PAGE_ID)!
  );
}

export function isSettingsPageId(value: string): value is SettingsPageId {
  return SETTINGS_PAGE_MAP.has(value as SettingsPageId);
}