import type { ComponentType } from "react";
import type { RecoverableLazyComponent } from "@/utils/lazyWithRecovery";

export type { ToolCategory } from "./categories";
export type { ToolCategoryMeta } from "./categories";

export type WidthMode = "default" | "wide" | "full";

export interface FileDropCapability {
  accept: string;
}

export interface ToolCapabilities {
  fileDrop?: FileDropCapability;
}

export interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  category: import("./categories").ToolCategory;
  icon: ComponentType;
  keywords: string[];
  component: RecoverableLazyComponent<ComponentType>;
  isNew?: boolean;
  createdAt?: Date;
  widthMode?: WidthMode;
  /** 窄容器（mini / 容器宽 ≤ MINI_DENSITY_BREAKPOINT）下仍可用 */
  miniFriendly?: boolean;
  capabilities?: ToolCapabilities;
}

export interface ToolCategoryGroup {
  category: import("./categories").ToolCategory;
  meta: import("./categories").ToolCategoryMeta;
  tools: ToolDefinition[];
}
