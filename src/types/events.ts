import type {
  SettingProviderChangedPayloadDto,
  ShortcutWarningPayloadDto,
} from "./protocol.contract";

export type SettingProviderChangedPayload = SettingProviderChangedPayloadDto;

export type ShortcutWarningPayload = ShortcutWarningPayloadDto;

export enum SettingAction {
  ProviderSaved = "provider-saved",
  ProviderDeleted = "provider-deleted",
  ThemeChanged = "theme-changed",
  ConfPatched = "conf-patched",
  MascotManifestChanged = "mascot-manifest-changed",
  AgentModesChanged = "agent-modes-changed",
}

export enum SettingEvent {
  ProviderChanged = "setting:provider-changed",
  ThemeChanged = "setting:theme-changed",
  ConfPatched = "setting:conf-patched",
  MascotManifestChanged = "setting:mascot-manifest-changed",
  AgentModesChanged = "setting:agent-modes-changed",
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
  WindowModeChanged = "ui:window-mode-changed",
  TimekeeperFinished = "timekeeper:finished",
  ScreenshotSession = "screenshot-session",
  ScreenshotPreparing = "screenshot-preparing",
  ScreenshotError = "screenshot-error",
}
