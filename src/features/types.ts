import type { ComponentType } from "react";
import type { SettingsPageDefinition } from "@/settings/core/types/pageTypes";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import type { SettingSectionDef } from "@/settings/core/field/fieldRegistry";

export type SidebarSlot = "after-tools" | "before-settings";

export interface FeatureRoute {
  path: string;
  load: () => Promise<{ default: ComponentType }>;
  boundary: string;
  order?: number;
}

export interface FeatureSidebarEntry {
  label: string;
  icon: ComponentType;
  slot: SidebarSlot;
  key?: string;
  order?: number;
}

export interface FeatureCategory {
  id: string;
  label: string;
  order?: number;
}

export interface FeatureSettings {
  pages?: readonly SettingsPageDefinition[];
  sections?: readonly SettingSectionDef[];
  fields?: readonly ExtensionSettingDefinition[];
}

export interface FeatureModule {
  id: string;
  order?: number;
  routes?: readonly FeatureRoute[];
  sidebar?: readonly FeatureSidebarEntry[];
  categories?: readonly FeatureCategory[];
  settings?: FeatureSettings;
}
