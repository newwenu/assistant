import type { ComponentType, FC } from "react";
import type { RecoverableLazyComponent } from "@/utils/lazyWithRecovery";

export type SettingsCategoryId = string & {};

export type SettingsPageId = string & {};

export type PageVisibility = "sidebar" | "deepLinkOnly";

export interface SettingsPageManifest {
  id: SettingsPageId;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
  icon: FC;
  order: number;
  navParent?: string;
  visibility: PageVisibility;
  emptyPage: boolean;
  component: RecoverableLazyComponent<ComponentType>;
}

export type SettingsPageDefinition = Omit<
  SettingsPageManifest,
  "component" | "visibility" | "emptyPage"
> & {
  load: () => Promise<{ default: ComponentType }>;
  visibility?: PageVisibility;
  emptyPage?: boolean;
};
