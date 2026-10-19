import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./types/pageTypes";
import { lazyWithRecovery } from "@/utils/lazyWithRecovery";

type SettingsPageDefinition = Omit<SettingsPageManifest, "component"> & {
  load: () => Promise<{ default: import("react").ComponentType }>;
};

export function definePage(
  definition: SettingsPageDefinition,
): SettingsPageManifest {
  const component = lazyWithRecovery(definition.load);
  return { ...definition, component };
}

const general = definePage({
  id: "general",
  categoryId: "general",
  label: "common.navigation.general.title",
  keywords: ["language", "launch", "tray", "top", "startup"],
  load: () =>
    import("../pages/GeneralSettings").then((m) => ({
      default: m.GeneralSettings,
    })),
});

const appearance = definePage({
  id: "appearance",
  categoryId: "general",
  label: "common.navigation.appearance.title",
  keywords: ["mascot", "渲染", "颜色"],
  load: () =>
    import("../pages/AppearanceSettings").then((m) => ({
      default: m.AppearanceSettings,
    })),
});

const shortcuts = definePage({
  id: "shortcuts",
  categoryId: "general",
  label: "common.navigation.shortcuts.title",
  keywords: ["keyboard", "shortcut", "keybinding", "hotkey"],
  load: () =>
    import("../pages/ShortcutSettings").then((m) => ({
      default: m.ShortcutSettings,
    })),
});

const providers = definePage({
  id: "providers",
  categoryId: "provider",
  label: "common.navigation.providers.title",
  keywords: ["provider", "api", "model", "openai", "preset"],
  load: () =>
    import("../provider/ProviderManager").then((m) => ({
      default: m.ProviderManager,
    })),
});

const agents = definePage({
  id: "agents",
  categoryId: "provider",
  label: "common.navigation.agents.title",
  keywords: ["agent", "mode", "kind", "quick", "deep"],
  load: () =>
    import("../pages/AgentModeSettings").then((m) => ({
      default: m.AgentModeSettings,
    })),
});

const about = definePage({
  id: "about",
  categoryId: "about",
  label: "common.navigation.about.title",
  keywords: ["version", "rustc", "build", "about"],
  load: () =>
    import("../pages/AboutSettings").then((m) => ({
      default: m.AboutSettings,
    })),
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
