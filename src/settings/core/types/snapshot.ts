import type { ComponentType } from "react";
import type {
  PageVisibility,
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "../field/fieldRegistry";
import type { ConfSlot } from "./confSlot";

export type SidebarSlot =
  "after-tools" | "before-history" | "after-history" | "before-settings";

export interface SnapshotRoute {
  path: string;
  element: ComponentType;
  boundary: string;
  order: number;
  navParent?: string;
  visibility: PageVisibility;
  keywords: readonly string[];
}

export interface SnapshotSidebarEntry {
  key: string;
  label: string;
  icon: ComponentType;
  slot: SidebarSlot;
  order: number;
}

export interface CategoryInput {
  id: SettingsCategoryId;
  label: string;
  order: number;
}

export interface SnapshotCategory extends CategoryInput {
  pages: readonly SettingsPageManifest[];
}

export type SnapshotPhase = "builtin" | "bound";

export type ViolationRule =
  | "duplicate-page-id"
  | "duplicate-category-id"
  | "duplicate-section-id"
  | "duplicate-field-key"
  | "duplicate-route-path"
  | "duplicate-sidebar-key"
  | "duplicate-conf-group"
  | "unknown-page-category"
  | "unknown-section-page"
  | "unknown-field-section"
  | "unknown-conf-group"
  | "unknown-conf-page"
  | "conf-group-without-page"
  | "conf-page-without-fields"
  | "unknown-nav-parent"
  | "orphan-sidebar-key"
  | "deep-link-with-nav-parent"
  | "page-without-sections";

export interface Violation {
  rule: ViolationRule;
  target: string;
  message: string;
}

export interface SnapshotSource {
  pages: readonly SettingsPageManifest[];
  categories: readonly CategoryInput[];
  sections: readonly SettingSectionDef[];
  fields: readonly SettingDefinition[];
  routes: readonly SnapshotRoute[];
  sidebar: readonly SnapshotSidebarEntry[];
  confs: readonly ConfSlot[];
}

export interface SettingsExtensionInput {
  id: string;
  routes: readonly SnapshotRoute[];
  sidebar: readonly SnapshotSidebarEntry[];
  categories: readonly CategoryInput[];
  pages: readonly SettingsPageManifest[];
  sections: readonly SettingSectionDef[];
  fields: readonly SettingDefinition[];
  confs: readonly ConfSlot[];
}

export interface SettingsSnapshot {
  version: number;
  phase: SnapshotPhase;
  pages: readonly SettingsPageManifest[];
  pageMap: ReadonlyMap<SettingsPageId, SettingsPageManifest>;
  categories: readonly SnapshotCategory[];
  categoryMap: ReadonlyMap<SettingsCategoryId, SnapshotCategory>;
  sections: readonly SettingSectionDef[];
  sectionMap: ReadonlyMap<string, SettingSectionDef>;
  sectionsByPage: ReadonlyMap<string, readonly SettingSectionDef[]>;
  fields: readonly SettingDefinition[];
  fieldsByGroup: ReadonlyMap<string, readonly SettingDefinition[]>;
  fieldsBySection: ReadonlyMap<string, readonly SettingDefinition[]>;
  routes: readonly SnapshotRoute[];
  sidebar: readonly SnapshotSidebarEntry[];
  confPageMap: ReadonlyMap<string, SettingsPageId>;
}

export const EMPTY_EXTENSION_INPUT: SettingsExtensionInput = {
  id: "empty",
  routes: [],
  sidebar: [],
  categories: [],
  pages: [],
  sections: [],
  fields: [],
  confs: [],
};
