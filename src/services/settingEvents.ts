import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  SettingEvent,
  SettingAction,
  type SettingEventPayload,
} from "../types/events";
import { useProviderStore } from "../stores/providerStore";
import { useThemeStore } from "../stores/themeStore";

export function subscribeSettingEvents(): () => void {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const track = (promise: Promise<UnlistenFn>) => {
    promise
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      })
      .catch((error) => {
        console.debug("[setting-events] listener unavailable:", error);
      });
  };

  track(
    listen<SettingEventPayload>(SettingEvent.ProviderChanged, ({ payload }) => {
      if (disposed) return;
      switch (payload.action) {
        case SettingAction.ProviderSaved:
        case SettingAction.ProviderDeleted:
          useProviderStore.getState().loadProviders();
          break;
      }
    }),
  );

  track(
    listen<SettingEventPayload>(SettingEvent.ThemeChanged, ({ payload }) => {
      if (disposed) return;
      if (payload.action === SettingAction.ThemeChanged) {
        useThemeStore.setState({
          resolved: payload.value as "light" | "dark",
        });
      }
    }),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}