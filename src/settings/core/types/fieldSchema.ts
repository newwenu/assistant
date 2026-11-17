import type { FC } from "react";

export type SettingType =
  | "boolean"
  | "string"
  | "number"
  | "color"
  | "slider"
  | "select"
  | "textarea"
  | "asset-picker"
  | "keybinding"
  | "secret"
  | "readonly";

export interface SelectOption {
  label: string;
  value: string;
}

export interface AssetManifestEditorProps {
  assetType: string;
}

export interface AssetConfig {
  assetType: string;
  fileFilter: { name: string; extensions: string[] };
  urlInput?: boolean;
  /** 素材伴生声明的编辑入口；仅当 `src` 指向本地安装素材时渲染。 */
  manifestEditor?: FC<AssetManifestEditorProps>;
}

export interface FieldContext {
  mode: "add" | "edit";
  formValues: Record<string, unknown>;
  entity?: unknown;
  preset?: unknown;
}

export type DynamicString = string | ((ctx: FieldContext) => string);

export interface FieldActionCallbacks {
  setFieldOptions: (options: SelectOption[]) => void;
  setFieldValue: (value: unknown) => void;
}

export interface FieldAction {
  key: string;
  label: string;
  icon?: FC;
  onClick: (
    ctx: FieldContext,
    callbacks: FieldActionCallbacks,
  ) => void | Promise<void>;
}

export interface FieldSchema {
  key: string;
  type: SettingType;
  label: string;
  description?: DynamicString;
  extra?: DynamicString;
  placeholder?: DynamicString;
  options?: SelectOption[];
  loadOptions?: (ctx: FieldContext) => Promise<SelectOption[]>;
  actions?: FieldAction[];
  min?: number;
  max?: number;
  step?: number;
  marks?: Record<number, string>;
  rows?: number;
  allowClear?: boolean;
  valuePropName?: string;
  customRender?: FC<FieldRenderProps>;
  assetConfig?: AssetConfig;
  visible?: (formValues: Record<string, unknown>) => boolean;
  maskedValue?: string;
  keepIfEmpty?: boolean;
  copyable?: boolean;
  loadValue?: (ctx: FieldContext) => Promise<unknown>;
}

export interface FieldRenderProps {
  value?: unknown;
  onChange?: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  schema: FieldSchema;
  conf?: Record<string, unknown>;
  disabled?: boolean;
  fieldContext?: FieldContext;
}

export interface SettingFieldProps {
  schema: FieldSchema;
  value: unknown;
  onChange: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  conf?: Record<string, unknown>;
  disabled?: boolean;
  fieldContext?: FieldContext;
}
