import type { ComponentType } from "react";
import { lazyWithRecovery } from "@/utils/lazyWithRecovery";
import { definePage } from "@/settings/core/definePage";
import { insertUnique } from "@/settings/core/insertUnique";
import type { SettingsPageManifest } from "@/settings/core/types/pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "@/settings/core/field/fieldRegistry";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import type { ConfSlot } from "@/settings/core/types/confSlot";
import type {
  FeatureCategory,
  FeatureModule,
  FeatureRoute,
  FeatureSidebarEntry,
  PageVisibility,
  SidebarSlot,
} from "./types";

export interface PreparedRoute {
  path: string;
  element: ComponentType;
  boundary: string;
  order: number;
  navParent?: string;
  visibility: PageVisibility;
  keywords: readonly string[];
}

export interface PreparedSidebarEntry {
  key: string;
  label: string;
  icon: ComponentType;
  slot: SidebarSlot;
  order: number;
}

export interface PreparedCategory {
  id: string;
  label: string;
  order: number;
}

export interface AggregatedFeatures {
  routes: readonly PreparedRoute[];
  sidebar: readonly PreparedSidebarEntry[];
  categories: readonly PreparedCategory[];
  pages: readonly SettingsPageManifest[];
  sections: readonly SettingSectionDef[];
  fields: readonly SettingDefinition[];
  confs: readonly ConfSlot[];
}

function toSettingDefinition(
  def: ExtensionSettingDefinition,
): SettingDefinition {
  const { customRenderLoad, ...rest } = def;
  if (!customRenderLoad) return rest;
  return { ...rest, customRender: lazyWithRecovery(customRenderLoad) };
}

function byOrder<T extends { order: number }>(
  a: T,
  b: T,
  tieBreak: (x: T, y: T) => number,
): number {
  if (a.order !== b.order) return a.order - b.order;
  return tieBreak(a, b);
}

export function aggregate(
  modules: readonly FeatureModule[],
): AggregatedFeatures {
  const routes: PreparedRoute[] = [];
  const routePaths = new Set<string>();
  const sidebar: PreparedSidebarEntry[] = [];
  const sidebarKeys = new Set<string>();
  const categories: PreparedCategory[] = [];
  const pages: SettingsPageManifest[] = [];
  const pageIds = new Set<string>();
  const sections: SettingSectionDef[] = [];
  const sectionIds = new Set<string>();
  const fields: SettingDefinition[] = [];
  const fieldKeys = new Set<string>();
  const confs: ConfSlot[] = [];
  const confGroups = new Set<string>();

  modules.forEach((mod) => {
    (mod.routes ?? []).forEach((route: FeatureRoute, i) => {
      insertUnique(
        routes,
        routePaths,
        route.path,
        {
          path: route.path,
          element: lazyWithRecovery(route.load),
          boundary: route.boundary,
          order: route.order ?? i,
          navParent: route.navParent,
          visibility: route.visibility ?? "sidebar",
          keywords: route.keywords ?? [],
        },
        "route path",
      );
    });

    (mod.sidebar ?? []).forEach((entry: FeatureSidebarEntry, i) => {
      insertUnique(
        sidebar,
        sidebarKeys,
        entry.key,
        {
          key: entry.key,
          label: entry.label,
          icon: entry.icon,
          slot: entry.slot,
          order: entry.order ?? i,
        },
        "sidebar key",
      );
    });

    (mod.categories ?? []).forEach((cat: FeatureCategory, i) => {
      categories.push({
        id: cat.id,
        label: cat.label,
        order: cat.order ?? i,
      });
    });

    (mod.settings?.pages ?? []).forEach((page) => {
      const manifest = definePage(page);
      insertUnique(pages, pageIds, manifest.id, manifest, "page id");
    });
    (mod.settings?.sections ?? []).forEach((section) => {
      insertUnique(sections, sectionIds, section.id, section, "section id");
    });
    (mod.settings?.fields ?? []).forEach((field) => {
      const def = toSettingDefinition(field);
      insertUnique(fields, fieldKeys, def.key, def, "setting key");
    });
    (mod.confs ?? []).forEach((slot) => {
      insertUnique(confs, confGroups, slot.group, slot, "conf group");
    });
  });

  const result: AggregatedFeatures = {
    routes: routes.sort((a, b) =>
      byOrder(a, b, (x, y) => x.path.localeCompare(y.path)),
    ),
    sidebar: sidebar.sort((a, b) =>
      byOrder(a, b, (x, y) => x.key.localeCompare(y.key)),
    ),
    categories: categories.sort((a, b) =>
      byOrder(a, b, (x, y) => x.id.localeCompare(y.id)),
    ),
    pages: pages.sort((a, b) =>
      byOrder(a, b, (x, y) => x.id.localeCompare(y.id)),
    ),
    sections,
    fields,
    confs,
  };

  return result;
}
