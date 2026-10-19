import type { ComponentType } from "react";
import type { RecoverableLazyComponent } from "@/utils/lazyWithRecovery";

export type SettingsCategoryId = "general" | "provider" | "about";

export type SettingsPageId =
  "general" | "appearance" | "shortcuts" | "providers" | "agents" | "about";

export interface SettingsPageManifest {
  id: SettingsPageId;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
  component: RecoverableLazyComponent<ComponentType>;
}
