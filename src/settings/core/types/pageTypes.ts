import type { ComponentType, FC } from "react";
import type { RecoverableLazyComponent } from "@/utils/lazyWithRecovery";

export type SettingsCategoryId = string & {};

export type SettingsPageId = string & {};

export interface SettingsPageManifest {
  id: SettingsPageId;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
  icon: FC;
  order: number;
  component: RecoverableLazyComponent<ComponentType>;
}

export type SettingsPageDefinition = Omit<SettingsPageManifest, "component"> & {
  load: () => Promise<{ default: ComponentType }>;
};
