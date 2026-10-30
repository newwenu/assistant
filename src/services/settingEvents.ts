import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  SettingEvent,
  SettingAction,
  type SettingEventPayload,
} from "../types/events";
import { useProviderStore } from "../stores/providerStore";
import { useThemeStore } from "../stores/themeStore";
import { useSettingsStore } from "../stores/settingsStore";
import { createLogger } from "./logger";
import { applyLocaleChange } from "@/i18n/localeApplier";

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
      if (payload.action === SettingAction.ThemeChanged) {
        useThemeStore.setState({
          resolved: payload.value as "light" | "dark",
        });
      }
    }),
  );

  track(
    listen<SettingEventPayload>(SettingEvent.ConfPatched, ({ payload }) => {
      if (disposed) return;
      if (payload.action === SettingAction.ConfPatched) {
        const { group, patch } = (payload.value as {
          group: string;
          patch: Record<string, unknown>;
        }) ?? { group: "", patch: {} };
        const store = useSettingsStore.getState();
        if (group === "app") {
          void store.loadAppConf().then(async () => {
            if ("language" in patch) {
              const lang =
                useSettingsStore.getState().appConf?.language ?? "zh-CN";
              await applyLocaleChange(lang);
            }
          });
        } else if (group === "orb") void store.loadOrbConf();
        else if (group === "mascot") void store.loadMascotConf();
        else if (group === "vault") void store.loadVaultConf();
      }
    }),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
