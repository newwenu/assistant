export enum SettingAction {
  ProviderSaved = "provider-saved",
  ProviderDeleted = "provider-deleted",
  ThemeChanged = "theme-changed",
  AppConfChanged = "app-conf-changed",
  OrbConfChanged = "orb-conf-changed",
}

export enum SettingEvent {
  ProviderChanged = "setting:provider-changed",
  ThemeChanged = "setting:theme-changed",
  AppConfChanged = "setting:app-conf-changed",
  OrbConfChanged = "setting:orb-conf-changed",
}

export interface SettingEventPayload {
  action: SettingAction;
  id?: string;
  value?: unknown;
}
