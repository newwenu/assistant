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
  | "keybinding";

export interface SelectOption {
  label: string;
  value: string;
}

export interface AssetConfig {
  assetType: string;
  fileFilter: { name: string; extensions: string[] };
  urlInput?: boolean;
}

export interface FieldSchema {
  key: string;
  type: SettingType;
  label: string;
  description?: string;
  extra?: string;
  placeholder?: string;
  options?: SelectOption[];
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
}

export interface FieldRenderProps {
  value?: unknown;
  onChange?: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  schema: FieldSchema;
  conf?: Record<string, unknown>;
  disabled?: boolean;
}

export interface SettingFieldProps {
  schema: FieldSchema;
  value: unknown;
  onChange: (value: unknown) => void;
  onChangeComplete?: (value: unknown) => void;
  conf?: Record<string, unknown>;
  disabled?: boolean;
}
