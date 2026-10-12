import type { FC } from "react";

export type SettingType =
  | "boolean"
  | "string"
  | "number"
  | "color"
  | "slider"
  | "select"
  | "asset-picker"
  | "keybinding";

export type ConfGroup = "app" | "orb" | "mascot";

export interface SelectOption {
  label: string;
  value: string;
}

export interface AssetConfig {
  assetType: string;
  fileFilter: { name: string; extensions: string[] };
  urlInput?: boolean;
}

export interface SettingSectionDef {
  id: string;
  pageId: string;
  title: string;
  description?: string;
  keywords?: string[];
}

export interface SettingDefinition {
  key: string;
  group: ConfGroup;
  type: SettingType;
  default: unknown;
  label: string;
  section?: string;
  description?: string;
  sideEffect?: string;
  visible?: (conf: Record<string, unknown>) => boolean;
  min?: number;
  max?: number;
  step?: number;
  options?: SelectOption[];
  marks?: Record<number, string>;
  extra?: string;
  valuePropName?: string;
  saveMessage?: string | false;
  customRender?: FC<SettingFieldCustomProps>;
  pageId?: string;
  readValue?: (conf: Record<string, unknown>) => unknown;
  buildPatch?: (
    value: unknown,
    conf: Record<string, unknown>,
  ) => Record<string, unknown>;
  assetConfig?: AssetConfig;
}

export interface SettingFieldCustomProps {
  value?: unknown;
  onChange?: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  def: SettingDefinition;
  conf?: Record<string, unknown>;
}

export interface SettingFieldProps {
  def: SettingDefinition;
  value: unknown;
  onChange: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  conf?: Record<string, unknown>;
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
]);
