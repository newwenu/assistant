import type { ComponentType, FC } from "react";
import type { RecoverableLazyComponent } from "@/utils/lazyWithRecovery";

export type SettingsCategoryId = "general" | "provider" | "vault" | "about";

export type SettingsPageId =
  | "overview"
  | "general"
  | "appearance"
  | "shortcuts"
  | "providers"
  | "agents"
  | "storage"
  | "vault"
  | "about";

export interface SettingsPageManifest {
  id: SettingsPageId;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
  icon: FC;
  component: RecoverableLazyComponent<ComponentType>;
}
