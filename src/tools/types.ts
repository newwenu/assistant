import type { ComponentType } from "react";

export type { ToolCategory } from "./categories";
export type { ToolCategoryMeta } from "./categories";

export type WidthMode = "default" | "wide" | "full";

export interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  category: import("./categories").ToolCategory;
  icon: ComponentType;
  keywords: string[];
  component: React.LazyExoticComponent<ComponentType>;
  isNew?: boolean;
  createdAt?: Date;
  widthMode?: WidthMode;
}

export interface ToolCategoryGroup {
  category: import("./categories").ToolCategory;
  meta: import("./categories").ToolCategoryMeta;
  tools: ToolDefinition[];
}
