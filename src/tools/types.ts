import type { ComponentType } from "react";

export type ToolCategory =
  "crypto" | "converter" | "web" | "dev" | "text" | "math" | "daily" | "media";

export type WidthMode = "default" | "wide" | "full";

export interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  category: ToolCategory;
  icon: ComponentType;
  keywords: string[];
  component: React.LazyExoticComponent<ComponentType>;
  isNew?: boolean;
  createdAt?: Date;
  widthMode?: WidthMode;
}

export interface ToolCategoryMeta {
  key: ToolCategory;
  label: string;
  icon: ComponentType;
  order: number;
}

export interface ToolCategoryGroup {
  category: ToolCategory;
  meta: ToolCategoryMeta;
  tools: ToolDefinition[];
}
