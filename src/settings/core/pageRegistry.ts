import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./types/pageTypes";
import {
  SettingOutlined,
  FormatPainterOutlined,
  ThunderboltOutlined,
  ApiOutlined,
  RobotOutlined,
  DatabaseOutlined,
  InfoCircleOutlined,
} from "@ant-design/icons";
import { definePage } from "./definePage";
import { getFeatures } from "@/features/registry";
import type { AggregatedFeatures } from "@/features/aggregate";

const overview = definePage({
  id: "overview",
  categoryId: "general",
  label: "common.navigation.settings",
  keywords: ["overview", "dashboard", "总览", "仪表盘"],
  icon: SettingOutlined,
  order: 0,
  load: () =>
    import("../overview/SettingsOverview").then((m) => ({
      default: m.SettingsOverview,
    })),
});

const general = definePage({
  id: "general",
  categoryId: "general",
  label: "common.navigation.general.title",
  keywords: ["language", "launch", "tray", "top", "startup"],
  icon: SettingOutlined,
  order: 10,
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
  icon: FormatPainterOutlined,
  order: 20,
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
  icon: ThunderboltOutlined,
  order: 30,
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
  icon: ApiOutlined,
  order: 40,
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
  icon: RobotOutlined,
  order: 50,
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
  icon: InfoCircleOutlined,
  order: 70,
  load: () =>
    import("../pages/AboutSettings").then((m) => ({
      default: m.AboutSettings,
    })),
});

const storage = definePage({
  id: "storage",
  categoryId: "general",
  label: "common.navigation.storage.title",
  keywords: ["storage", "attachment", "orphan", "disk", "存储", "附件", "孤儿"],
  icon: DatabaseOutlined,
  order: 60,
  load: () =>
    import("../pages/StorageSettings").then((m) => ({
      default: m.StorageSettings,
    })),
});

const CORE_PAGES: readonly SettingsPageManifest[] = [
  general,
  appearance,
  shortcuts,
  providers,
  agents,
  storage,
  about,
];

export interface SettingsCategory {
  id: SettingsCategoryId;
  label: string;
  pages: readonly SettingsPageManifest[];
}

interface CategoryDefinition {
  id: SettingsCategoryId;
  label: string;
  order: number;
}

const CORE_CATEGORIES: readonly CategoryDefinition[] = [
  { id: "general", label: "common.navigation.category.general", order: 0 },
  { id: "provider", label: "common.navigation.category.provider", order: 10 },
  { id: "about", label: "common.navigation.category.about", order: 30 },
];

export const DEFAULT_SETTINGS_PAGE_ID: SettingsPageId = "overview";

interface DerivedRegistry {
  features: AggregatedFeatures;
  pages: readonly SettingsPageManifest[];
  pageMap: Map<SettingsPageId, SettingsPageManifest>;
  categories: readonly SettingsCategory[];
}

let derived: DerivedRegistry | null = null;

function derive(): DerivedRegistry {
  const features = getFeatures();
  if (derived && derived.features === features) return derived;

  const pages = [...CORE_PAGES, ...features.pages].sort(
    (a, b) => a.order - b.order,
  );

  const pageMap = new Map<SettingsPageId, SettingsPageManifest>(
    [overview, ...pages].map((p) => [p.id, p]),
  );

  const categories = [...CORE_CATEGORIES, ...features.categories]
    .sort((a, b) => a.order - b.order)
    .map((category) => ({
      id: category.id,
      label: category.label,
      pages: pages.filter((p) => p.categoryId === category.id),
    }))
    .filter((c) => c.pages.length > 0);

  derived = { features, pages, pageMap, categories };
  return derived;
}

export function getSettingsPages(): readonly SettingsPageManifest[] {
  return derive().pages;
}

export function getSettingsDetailPages(): readonly SettingsPageManifest[] {
  return derive().pages;
}

export function getSettingsCategories(): readonly SettingsCategory[] {
  return derive().categories;
}

export function getSettingsPageMap(): ReadonlyMap<
  SettingsPageId,
  SettingsPageManifest
> {
  return derive().pageMap;
}

export function getSettingsPageManifest(
  pageId: SettingsPageId,
): SettingsPageManifest {
  const map = derive().pageMap;
  return map.get(pageId) ?? map.get(DEFAULT_SETTINGS_PAGE_ID)!;
}

export function isSettingsPageId(value: string): value is SettingsPageId {
  return derive().pageMap.has(value);
}
