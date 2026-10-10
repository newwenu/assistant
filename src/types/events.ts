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
