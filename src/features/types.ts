import type { ComponentType } from "react";
import type {
  PageVisibility,
  SettingsPageDefinition,
} from "@/settings/core/types/pageTypes";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import type { SettingSectionDef } from "@/settings/core/field/fieldRegistry";
import type { SidebarSlot } from "@/settings/core/types/snapshot";
import type { ConfSlot } from "@/settings/core/types/confSlot";

export type { PageVisibility, SidebarSlot };

export interface FeatureRoute {
  path: string;
  load: () => Promise<{ default: ComponentType }>;
  boundary: string;
  order?: number;
  navParent?: string;
  visibility?: PageVisibility;
  keywords?: readonly string[];
}

export interface FeatureSidebarEntry {
  key: string;
  label: string;
  icon: ComponentType;
  slot: SidebarSlot;
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

export interface FeatureIpc {
  namespace: string;
}

export interface FeatureModule {
  id: string;
  order?: number;
  routes?: readonly FeatureRoute[];
  sidebar?: readonly FeatureSidebarEntry[];
  categories?: readonly FeatureCategory[];
  settings?: FeatureSettings;
  confs?: readonly ConfSlot[];
  ipc?: FeatureIpc;
}
