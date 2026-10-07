import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  SettingAction,
  SettingEvent,
  type SettingEventPayload,
} from "../types/events";
import { WINDOW_MAIN, WINDOW_ORB } from "../config/constants";
import { useProviderStore } from "../stores/providerStore";
import { useThemeStore } from "../stores/themeStore";
import { useSettingsStore } from "../stores/settingsStore";

async function emitSettingEvent(
  event: SettingEvent,
  payload: SettingEventPayload,
) {
  const targets = [WINDOW_MAIN, WINDOW_ORB];
  for (const label of targets) {
    const win = await WebviewWindow.getByLabel(label);
    if (win) {
      win
        .emit(event, payload)
        .catch((err: unknown) =>
          console.warn(`[settingsDispatcher] emit to ${label} failed:`, err),
        );
    }
  }
}

export async function dispatchProviderSaved(provider: Record<string, unknown>) {
  await invoke("save_provider", { provider });
  useProviderStore.getState().loadProviders();
  await emitSettingEvent(SettingEvent.ProviderChanged, {
    action: SettingAction.ProviderSaved,
    id: provider.id as string,
    value: provider,
  });
}

export async function dispatchProviderDeleted(id: string) {
  await invoke("delete_provider", { id });
  useProviderStore.getState().loadProviders();
  await emitSettingEvent(SettingEvent.ProviderChanged, {
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
    resolved = sys as "light" | "dark";
  }
  useThemeStore.setState({ mode, resolved });
  await invoke("emit_theme_changed", { resolved });
  await emitSettingEvent(SettingEvent.ThemeChanged, {
    action: SettingAction.ThemeChanged,
    value: resolved,
  });
}

export async function dispatchAppConfChanged(conf: Record<string, unknown>) {
  await invoke("set_app_conf", { conf });
  useSettingsStore.getState().loadAppConf();
  await emitSettingEvent(SettingEvent.AppConfChanged, {
    action: SettingAction.AppConfChanged,
    value: conf,
  });
}

export async function dispatchOrbConfChanged(conf: Record<string, unknown>) {
  await invoke("set_orb_conf", { conf });
  useSettingsStore.getState().loadOrbConf();
  await emitSettingEvent(SettingEvent.OrbConfChanged, {
    action: SettingAction.OrbConfChanged,
    value: conf,
  });
}

export async function dispatchTemplateSaved(template: Record<string, unknown>) {
  await invoke("save_prompt_template", { template });
  await emitSettingEvent(SettingEvent.TemplateChanged, {
    action: SettingAction.TemplateSaved,
    id: template.id as string,
    value: template,
  });
}
