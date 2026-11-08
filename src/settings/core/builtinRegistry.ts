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
import { insertUnique as insertUniqueEntry } from "./insertUnique";
import type {
  SettingDefinition,
  SettingSectionDef,
  SettingTypeRenderer,
} from "./field/fieldRegistry";
import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./types/pageTypes";
import type { ConfSlot } from "./types/confSlot";

export const DEFAULT_SETTINGS_PAGE_ID: SettingsPageId = "overview";

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
  emptyPage: true,
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
  emptyPage: true,
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
  emptyPage: true,
  load: () =>
    import("../pages/StorageSettings").then((m) => ({
      default: m.StorageSettings,
    })),
});

const builtinPages: readonly SettingsPageManifest[] = [
  general,
  appearance,
  shortcuts,
  providers,
  agents,
  storage,
  about,
];

export interface BuiltinCategory {
  id: SettingsCategoryId;
  label: string;
  order: number;
}

const builtinCategories: readonly BuiltinCategory[] = [
  { id: "general", label: "common.navigation.category.general", order: 0 },
  { id: "provider", label: "common.navigation.category.provider", order: 10 },
  { id: "about", label: "common.navigation.category.about", order: 30 },
];

const definitions: SettingDefinition[] = [];
const definitionKeys = new Set<string>();
const sections: SettingSectionDef[] = [];
const sectionIds = new Set<string>();
const confSlots: ConfSlot[] = [];
const confSlotGroups = new Set<string>();
const typeRenderers = new Map<string, SettingTypeRenderer>();

let builtinVersion = 0;

export function bumpBuiltinVersion(): void {
  builtinVersion += 1;
}

export function getBuiltinVersion(): number {
  return builtinVersion;
}

export function insertUnique<T>(
  list: T[],
  keys: Set<string>,
  key: string,
  item: T,
  kind: string,
): void {
  if (insertUniqueEntry(list, keys, key, item, kind)) bumpBuiltinVersion();
}

export function defineSetting(def: SettingDefinition): SettingDefinition {
  insertUnique(definitions, definitionKeys, def.key, def, "setting key");
  return def;
}

export function defineSection(sec: SettingSectionDef): SettingSectionDef {
  insertUnique(sections, sectionIds, sec.id, sec, "section id");
  return sec;
}

export function defineConfSlot(slot: ConfSlot): ConfSlot {
  insertUnique(confSlots, confSlotGroups, slot.group, slot, "conf group");
  return slot;
}

export function registerSettingType(
  type: string,
  renderer: SettingTypeRenderer,
): void {
  typeRenderers.set(type, renderer);
  bumpBuiltinVersion();
}

export function getSettingTypeRenderer(
  type: string,
): SettingTypeRenderer | undefined {
  return typeRenderers.get(type);
}

export function getOverviewPage(): SettingsPageManifest {
  return overview;
}

export function getBuiltinPages(): readonly SettingsPageManifest[] {
  return builtinPages;
}

export function getBuiltinCategories(): readonly BuiltinCategory[] {
  return builtinCategories;
}

export function getBuiltinDefinitions(): readonly SettingDefinition[] {
  return definitions;
}

export function getBuiltinSections(): readonly SettingSectionDef[] {
  return sections;
}

export function getBuiltinConfSlots(): readonly ConfSlot[] {
  return confSlots;
}

defineConfSlot({ group: "app", invoke: "get_app_conf", pageId: "general" });
defineConfSlot({ group: "orb", invoke: "get_orb_conf", pageId: "appearance" });
defineConfSlot({
  group: "mascot",
  invoke: "get_mascot_conf",
  pageId: "appearance",
});
