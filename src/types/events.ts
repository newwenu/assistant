import type {
  SettingProviderChangedPayloadDto,
  ShortcutWarningPayloadDto,
  ThemeChangedPayloadDto,
} from "./protocol.contract";

export type SettingProviderChangedPayload = SettingProviderChangedPayloadDto;

export type ShortcutWarningPayload = ShortcutWarningPayloadDto;

export type ThemeChangedPayload = Omit<ThemeChangedPayloadDto, "resolved"> & {
  resolved: "light" | "dark";
};

export enum SettingAction {
  ProviderSaved = "provider-saved",
  ProviderDeleted = "provider-deleted",
  ThemeChanged = "theme-changed",
  ConfPatched = "conf-patched",
}

export enum SettingEvent {
  ProviderChanged = "setting:provider-changed",
  ThemeChanged = "setting:theme-changed",
  ConfPatched = "setting:conf-patched",
}

export interface SettingEventPayload {
  action: SettingAction;
  id?: string;
  value?: unknown;
}

export enum UiEvent {
  ShortcutWarning = "shortcut-warning",
  Navigate = "navigate",
  ThemeChanged = "theme-changed",
  OrbNewConversation = "orb-new-conversation",
  OrbBlur = "orb-blur",
}
