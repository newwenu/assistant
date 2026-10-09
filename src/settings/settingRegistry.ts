import type { FC } from "react";

export type SettingType =
  | "boolean"
  | "string"
  | "number"
  | "color"
  | "slider"
  | "select"
  | "keybinding";

export type ConfGroup = "app" | "orb" | "mascot";

export interface SelectOption {
  label: string;
  value: string;
}

export interface SettingDefinition {
  key: string;
  group: ConfGroup;
  type: SettingType;
  default: unknown;
  label: string;
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
  customRender?: FC<SettingFieldCustomProps>;
}

export interface SettingFieldCustomProps {
  value?: unknown;
  onChange?: (value: unknown) => void;
  def: SettingDefinition;
}

export interface SettingFieldProps {
  def: SettingDefinition;
  value: unknown;
  onChange: (value: unknown) => void;
}

export interface SettingTypeRenderer {
  component: FC<SettingFieldProps>;
}

const definitions: SettingDefinition[] = [];
const typeRenderers = new Map<string, SettingTypeRenderer>();

export function defineSetting(def: SettingDefinition): SettingDefinition {
  definitions.push(def);
  return def;
}

export function getDefinitionsByGroup(group: ConfGroup): SettingDefinition[] {
  return definitions.filter((d) => d.group === group);
}

export function getAllDefinitions(): readonly SettingDefinition[] {
  return definitions;
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
