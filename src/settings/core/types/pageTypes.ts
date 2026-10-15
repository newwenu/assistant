import type { ComponentType } from "react";

export type SettingsCategoryId = "general" | "provider" | "about";

export type SettingsPageId =
  "general" | "appearance" | "shortcuts" | "providers" | "agents" | "about";

export interface SettingsPageManifest {
  id: SettingsPageId;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
  component: React.LazyExoticComponent<ComponentType>;
}
