import { getSettingsSnapshot } from "./snapshot";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "./field/fieldRegistry";

const EMPTY_FIELDS: readonly SettingDefinition[] = Object.freeze([]);
const EMPTY_SECTIONS: readonly SettingSectionDef[] = Object.freeze([]);

export function getAllDefinitions(): readonly SettingDefinition[] {
  return getSettingsSnapshot().fields;
}

export function getDefinitionsByGroup(
  group: string,
): readonly SettingDefinition[] {
  return getSettingsSnapshot().fieldsByGroup.get(group) ?? EMPTY_FIELDS;
}

export function getDefinitionsBySection(
  sectionId: string,
): readonly SettingDefinition[] {
  return getSettingsSnapshot().fieldsBySection.get(sectionId) ?? EMPTY_FIELDS;
}

export function getAllSections(): readonly SettingSectionDef[] {
  return getSettingsSnapshot().sections;
}

export function getSectionsByPage(
  pageId: string,
): readonly SettingSectionDef[] {
  return getSettingsSnapshot().sectionsByPage.get(pageId) ?? EMPTY_SECTIONS;
}

export function getSectionById(id: string): SettingSectionDef | undefined {
  return getSettingsSnapshot().sectionMap.get(id);
}
