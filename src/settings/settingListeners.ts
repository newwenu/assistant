import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  SettingEvent,
  SettingAction,
  type SettingEventPayload,
} from "../types/events";
import { useProviderStore } from "../stores/providerStore";
import { useThemeStore } from "../stores/themeStore";

const unlisteners: UnlistenFn[] = [];

export async function initSettingListeners() {
  if (unlisteners.length > 0) return;

  const providerUnlisten = await listen<SettingEventPayload>(
    SettingEvent.ProviderChanged,
    ({ payload }) => {
      switch (payload.action) {
        case SettingAction.ProviderSaved:
        case SettingAction.ProviderDeleted:
          useProviderStore.getState().loadProviders();
          break;
      }
    },
  );
  unlisteners.push(providerUnlisten);

  const themeUnlisten = await listen<SettingEventPayload>(
    SettingEvent.ThemeChanged,
    ({ payload }) => {
      if (payload.action === SettingAction.ThemeChanged) {
        useThemeStore.setState({
          resolved: payload.value as "light" | "dark",
        });
      }
    },
  );
  unlisteners.push(themeUnlisten);
}

export async function cleanupSettingListeners() {
  for (const fn of unlisteners) {
    fn();
  }
  unlisteners.length = 0;
}
