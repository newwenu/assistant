import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  SettingAction,
  SettingEvent,
  type SettingEventPayload,
} from "@/types/events";
import { WINDOW_MAIN, WINDOW_ORB } from "@/config/constants";
import { useProviderStore } from "@/stores/providerStore";
import { useThemeStore } from "@/stores/themeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { MascotConf } from "@/stores/settingsStore";

export function buildRendererConfig(conf: MascotConf): Record<string, unknown> {
  return {
    idlePoseRotation: conf.rive_idle_pose_rotation,
    animated: conf.svg_animated,
    src: conf.gif_src,
    borderRadius: conf.gif_border_radius,
  };
}

export function buildMascotPatch(
  renderer: string,
  rendererConfig?: Record<string, unknown>,
): Record<string, unknown> {
  const patch: Record<string, unknown> = { renderer };
  if (rendererConfig) {
    if (rendererConfig.idlePoseRotation !== undefined)
      patch.rive_idle_pose_rotation = rendererConfig.idlePoseRotation;
    if (rendererConfig.animated !== undefined)
      patch.svg_animated = rendererConfig.animated;
    if (rendererConfig.src !== undefined) patch.gif_src = rendererConfig.src;
    if (rendererConfig.borderRadius !== undefined)
      patch.gif_border_radius = rendererConfig.borderRadius;
  }
  return patch;
}

async function emitSettingEventToPeers(
  event: SettingEvent,
  payload: SettingEventPayload,
) {
  const selfLabel = getCurrentWebview().label;
  const targets = [WINDOW_MAIN, WINDOW_ORB].filter(
    (label) => label !== selfLabel,
  );
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
  await emitSettingEventToPeers(SettingEvent.ProviderChanged, {
    action: SettingAction.ProviderSaved,
    id: provider.id as string,
    value: provider,
  });
}

export async function dispatchProviderDeleted(id: string) {
  await invoke("delete_provider", { id });
  useProviderStore.getState().loadProviders();
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
    resolved = sys as "light" | "dark";
  }
  useThemeStore.setState({ mode, resolved });
  await invoke("emit_theme_changed", { resolved });
  await emitSettingEventToPeers(SettingEvent.ThemeChanged, {
    action: SettingAction.ThemeChanged,
    value: resolved,
  });
}

export async function dispatchConfPatch(
  group: "app" | "orb" | "mascot",
  patch: Record<string, unknown>,
) {
  await invoke("patch_conf", { group, patch });
  const store = useSettingsStore.getState();
  if (group === "app") await store.loadAppConf();
  else if (group === "orb") await store.loadOrbConf();
  else if (group === "mascot") await store.loadMascotConf();
  await emitSettingEventToPeers(SettingEvent.ConfPatched, {
    action: SettingAction.ConfPatched,
    value: { group, patch },
  });
}
