import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { SettingAction, SettingEvent } from "@/types/events";
import { WINDOW_MAIN } from "@/config/constants";
import { useProviderStore } from "@/stores/providerStore";
import { useThemeStore } from "@/stores/themeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { providerService } from "@/services/providerService";
import { settingService } from "@/services/settingService";
import { createLogger } from "@/services/logger";
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";
import type { ProviderInput } from "@/settings/provider/types";
import { CONF_LOAD_KEY_MAP, type ConfGroup } from "./field/fieldRegistry";
import { emitSettingEventToPeers } from "@/services/crossWindowEvent";

const log = createLogger("settingsDispatcher");

export type ConfOf<G extends ConfGroup> = G extends "app"
  ? AppConf
  : G extends "orb"
    ? OrbConf
    : G extends "mascot"
      ? MascotConf
      : never;

export async function dispatchProviderSaved(provider: ProviderInput) {
  await providerService.saveProvider(provider);
  await useProviderStore.getState().loadProviders();
  await emitSettingEventToPeers(SettingEvent.ProviderChanged, {
    action: SettingAction.ProviderSaved,
    id: provider.id,
    value: provider,
  });
}

export async function dispatchProviderDeleted(id: string) {
  await providerService.deleteProvider(id);
  await useProviderStore.getState().loadProviders();
  await emitSettingEventToPeers(SettingEvent.ProviderChanged, {
    action: SettingAction.ProviderDeleted,
    id,
  });
}

export async function dispatchThemeChanged(mode: "system" | "light" | "dark") {
  let resolved: "light" | "dark";
  if (mode !== "system") {
    resolved = mode;
  } else {
    const mainWin = await WebviewWindow.getByLabel(WINDOW_MAIN);
    const sys = mainWin ? ((await mainWin.theme()) ?? "light") : "light";
    resolved = sys;
  }
  useThemeStore.setState({ mode, resolved });
  await emitSettingEventToPeers(SettingEvent.ThemeChanged, {
    action: SettingAction.ThemeChanged,
    value: resolved,
  });
}

export async function dispatchConfPatch<G extends ConfGroup>(
  group: G,
  patch: Partial<ConfOf<G>>,
) {
  try {
    await settingService.patchConf(group, patch);
    const store = useSettingsStore.getState();
    await store[CONF_LOAD_KEY_MAP[group]]();
    await emitSettingEventToPeers(SettingEvent.ConfPatched, {
      action: SettingAction.ConfPatched,
      value: { group, patch },
    });
  } catch (e) {
    log.error("dispatchConfPatch failed", e);
  }
}
