import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  SettingEvent,
  SettingAction,
  type SettingEventPayload,
} from "../types/events";
import { useProviderStore } from "../stores/providerStore";
import { useThemeStore } from "../stores/system/themeStore";
import { useSettingsStore } from "../stores/settingsStore";
import { createLogger } from "./system/logger";
import { isConfGroupAvailable } from "@/settings/core/runtime/confRuntime";

const log = createLogger("setting-events");

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
        log.debug("listener unavailable", error);
      });
  };

  track(
    listen<SettingEventPayload>(SettingEvent.ProviderChanged, ({ payload }) => {
      if (disposed) return;
      switch (payload.action) {
        case SettingAction.ProviderSaved:
        case SettingAction.ProviderDeleted:
          void useProviderStore.getState().loadProviders();
          break;
      }
    }),
  );

  track(
    listen<SettingEventPayload>(SettingEvent.ThemeChanged, ({ payload }) => {
      if (disposed) return;
      if (payload.action !== SettingAction.ThemeChanged) return;
      const value = payload.value;
      if (value !== "light" && value !== "dark") return;
      useThemeStore.setState({ resolved: value });
    }),
  );

  track(
    listen<SettingEventPayload>(SettingEvent.ConfPatched, ({ payload }) => {
      if (disposed) return;
      if (payload.action !== SettingAction.ConfPatched) return;
      const { group } = (payload.value as {
        group: string;
        patch: Record<string, unknown>;
      }) ?? { group: "", patch: {} };
      if (!isConfGroupAvailable(group)) return;
      void useSettingsStore.getState().loadConf(group);
    }),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
