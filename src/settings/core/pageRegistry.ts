import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./types/pageTypes";
import type { SnapshotCategory } from "./types/snapshot";
import { getSettingsSnapshot } from "./snapshot";
import { DEFAULT_SETTINGS_PAGE_ID } from "./builtinRegistry";

export type SettingsCategory = SnapshotCategory;

export { DEFAULT_SETTINGS_PAGE_ID };

export function getSettingsPages(): readonly SettingsPageManifest[] {
  return getSettingsSnapshot().pages;
}

export function getSettingsDetailPages(): readonly SettingsPageManifest[] {
  return getSettingsSnapshot().pages.filter(
    (page) => page.visibility !== "deepLinkOnly",
  );
}

export function getSettingsCategories(): readonly SettingsCategory[] {
  return getSettingsSnapshot().categories;
}

export function getSettingsPageMap(): ReadonlyMap<
  SettingsPageId,
  SettingsPageManifest
> {
  return getSettingsSnapshot().pageMap;
}

export function getSettingsPageManifest(
  pageId: SettingsPageId,
): SettingsPageManifest | undefined {
  return getSettingsSnapshot().pageMap.get(pageId);
}

export function isSettingsPageId(value: string): value is SettingsPageId {
  return getSettingsSnapshot().pageMap.has(value);
}

export type { SettingsCategoryId, SettingsPageId };
