import type { ComponentType } from "react";
import { lazyWithRecovery } from "@/utils/lazyWithRecovery";
import { definePage } from "@/settings/core/definePage";
import type { SettingsPageManifest } from "@/settings/core/types/pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "@/settings/core/field/fieldRegistry";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import type {
  FeatureCategory,
  FeatureModule,
  FeatureRoute,
  FeatureSidebarEntry,
  SidebarSlot,
} from "./types";

export interface PreparedRoute {
  path: string;
  element: ComponentType;
  boundary: string;
  order: number;
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

function assertUnique(features: AggregatedFeatures): void {
  if (!import.meta.env.DEV) return;

  const routePaths = new Set<string>();
  for (const route of features.routes) {
    if (routePaths.has(route.path)) {
      throw new Error(`[features] duplicate route path: ${route.path}`);
    }
    routePaths.add(route.path);
  }

  const fieldKeys = new Set<string>();
  for (const field of features.fields) {
    if (fieldKeys.has(field.key)) {
      throw new Error(`[features] duplicate setting key: ${field.key}`);
    }
    fieldKeys.add(field.key);
  }

  const sectionIds = new Set<string>();
  for (const section of features.sections) {
    if (sectionIds.has(section.id)) {
      throw new Error(`[features] duplicate section id: ${section.id}`);
    }
    sectionIds.add(section.id);
  }
}

export function aggregate(
  modules: readonly FeatureModule[],
): AggregatedFeatures {
  const routes: PreparedRoute[] = [];
  const sidebar: PreparedSidebarEntry[] = [];
  const categories: PreparedCategory[] = [];
  const pages: SettingsPageManifest[] = [];
  const sections: SettingSectionDef[] = [];
  const fields: SettingDefinition[] = [];

  modules.forEach((mod, index) => {
    const base = mod.order ?? index * 100;
    const defaultRoutePath = mod.routes?.[0]?.path ?? `/${mod.id}`;

    (mod.routes ?? []).forEach((route: FeatureRoute, i) => {
      routes.push({
        path: route.path,
        element: lazyWithRecovery(route.load),
        boundary: route.boundary,
        order: route.order ?? base + i,
      });
    });

    (mod.sidebar ?? []).forEach((entry: FeatureSidebarEntry, i) => {
      sidebar.push({
        key: entry.key ?? defaultRoutePath,
        label: entry.label,
        icon: entry.icon,
        slot: entry.slot,
        order: entry.order ?? base + i,
      });
    });

    (mod.categories ?? []).forEach((cat: FeatureCategory, i) => {
      categories.push({
        id: cat.id,
        label: cat.label,
        order: cat.order ?? base + i,
      });
    });

    (mod.settings?.pages ?? []).forEach((page) => pages.push(definePage(page)));
    (mod.settings?.sections ?? []).forEach((section) => sections.push(section));
    (mod.settings?.fields ?? []).forEach((field) =>
      fields.push(toSettingDefinition(field)),
    );
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
  };

  assertUnique(result);
  return result;
}
