import type { FC } from "react";
import type { ConfGroup } from "../types/confRegistry";
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
import {
  defineSetting,
  defineSection,
  registerSettingType,
  getSettingTypeRenderer,
} from "../registry/builtinRegistry";
import {
  getAllDefinitions,
  getDefinitionsByGroup,
  getDefinitionsBySection,
  getAllSections,
  getSectionsByPage,
  getSectionById,
} from "../runtime/query";

export {
  defineSetting,
  defineSection,
  registerSettingType,
  getSettingTypeRenderer,
  getAllDefinitions,
  getDefinitionsByGroup,
  getDefinitionsBySection,
  getAllSections,
  getSectionsByPage,
  getSectionById,
};

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

export const INTERMEDIATE_TYPES = new Set<SettingType>([
  "slider",
  "string",
  "number",
  "color",
  "textarea",
  "secret",
  "readonly",
]);
