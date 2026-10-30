import type { FC } from "react";
import type {
  SettingType,
  SelectOption,
  AssetConfig,
  FieldSchema,
  FieldRenderProps,
  SettingFieldProps,
  FieldContext,
  DynamicString,
  FieldAction,
  FieldActionCallbacks,
} from "../types/fieldSchema";

export type {
  SettingType,
  SelectOption,
  AssetConfig,
  FieldSchema,
  FieldRenderProps,
  SettingFieldProps,
  FieldContext,
  DynamicString,
  FieldAction,
  FieldActionCallbacks,
};

export const CONF_KEY_MAP = {
  app: "appConf",
  orb: "orbConf",
  mascot: "mascotConf",
  vault: "vaultConf",
} as const;

export const CONF_LOAD_KEY_MAP = {
  app: "loadAppConf",
  orb: "loadOrbConf",
  mascot: "loadMascotConf",
  vault: "loadVaultConf",
} as const;

export type ConfGroup = keyof typeof CONF_KEY_MAP;
export type ConfKey = (typeof CONF_KEY_MAP)[ConfGroup];
export type ConfLoadKey = (typeof CONF_LOAD_KEY_MAP)[ConfGroup];

export interface SettingSectionDef {
  id: string;
  pageId: string;
  title: string;
  description?: string;
  keywords?: string[];
}

export interface SettingDefinition extends FieldSchema {
  group: ConfGroup;
  default: unknown;
  section?: string;
  sideEffect?: string;
  visible?: (conf: Record<string, unknown>) => boolean;
  saveMessage?: string | false;
  pageId?: string;
  readValue?: (conf: Record<string, unknown>) => unknown;
  buildPatch?: (
    value: unknown,
    conf: Record<string, unknown>,
  ) => Record<string, unknown>;
}

export interface SettingTypeRenderer {
  component: FC<SettingFieldProps>;
}

const definitions: SettingDefinition[] = [];
const definitionKeys = new Set<string>();
const sections: SettingSectionDef[] = [];
const sectionIds = new Set<string>();
const typeRenderers = new Map<string, SettingTypeRenderer>();

export function defineSetting(def: SettingDefinition): SettingDefinition {
  if (!definitionKeys.has(def.key)) {
    definitionKeys.add(def.key);
    definitions.push(def);
  }
  return def;
}

export function defineSection(sec: SettingSectionDef): SettingSectionDef {
  if (!sectionIds.has(sec.id)) {
    sectionIds.add(sec.id);
    sections.push(sec);
  }
  return sec;
}

export function getDefinitionsByGroup(group: ConfGroup): SettingDefinition[] {
  return definitions.filter((d) => d.group === group);
}

export function getDefinitionsBySection(
  sectionId: string,
): SettingDefinition[] {
  return definitions.filter((d) => d.section === sectionId);
}

export function getAllDefinitions(): readonly SettingDefinition[] {
  return definitions;
}

export function getSectionsByPage(pageId: string): SettingSectionDef[] {
  return sections.filter((s) => s.pageId === pageId);
}

export function getSectionById(id: string): SettingSectionDef | undefined {
  return sections.find((s) => s.id === id);
}

export function getAllSections(): readonly SettingSectionDef[] {
  return sections;
}

export function registerSettingType(
  type: string,
  renderer: SettingTypeRenderer,
): void {
  typeRenderers.set(type, renderer);
}

export function getSettingTypeRenderer(
  type: string,
): SettingTypeRenderer | undefined {
  return typeRenderers.get(type);
}

export const INTERMEDIATE_TYPES = new Set<SettingType>([
  "slider",
  "string",
  "number",
  "color",
  "textarea",
  "secret",
  "readonly",
]);
