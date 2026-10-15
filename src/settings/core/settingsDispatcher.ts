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
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";
import type { ProviderInput } from "@/settings/provider/types";
import type { RendererConfig } from "@/mascot/core/types";
import type { ConfGroup } from "./field/fieldRegistry";

const CONF_KEY_MAP: Record<ConfGroup, "appConf" | "orbConf" | "mascotConf"> = {
  app: "appConf",
  orb: "orbConf",
  mascot: "mascotConf",
};

type ConfUnion = AppConf | OrbConf | MascotConf;

export type ConfOf<G extends ConfGroup> = G extends "app"
  ? AppConf
  : G extends "orb"
    ? OrbConf
    : G extends "mascot"
      ? MascotConf
      : never;

export function toConfRecord(
  conf: ConfUnion | null,
): Record<string, unknown> | null {
  if (!conf) return null;
  return conf as unknown as Record<string, unknown>;
}

export function optimisticConfUpdate(
  group: ConfGroup,
  key: string,
  value: unknown,
): void {
  const confKey = CONF_KEY_MAP[group];
  const store = useSettingsStore.getState();
  const current = store[confKey] as unknown as Record<string, unknown> | null;
  if (!current) return;
  useSettingsStore.setState({
    [confKey]: { ...current, [key]: value },
  });
}

export function buildRendererConfig(conf: MascotConf): RendererConfig {
  const renderer = conf.renderer ?? "rive";
  return {
    idlePoseRotation: conf.rive_idle_pose_rotation,
    src: renderer === "rive" ? conf.rive_src : conf.gif_src,
    animated: conf.svg_animated,
    borderRadius: conf.gif_border_radius,
  };
}

export function buildMascotPatch(
  renderer: string,
  rendererConfig?: RendererConfig,
): Partial<MascotConf> {
  const patch: Partial<MascotConf> = {
    renderer: renderer,
  };
  if (rendererConfig) {
    if (
      "idlePoseRotation" in rendererConfig &&
      rendererConfig.idlePoseRotation !== undefined
    )
      patch.rive_idle_pose_rotation = rendererConfig.idlePoseRotation;
    if ("animated" in rendererConfig && rendererConfig.animated !== undefined)
      patch.svg_animated = rendererConfig.animated;
    if ("src" in rendererConfig && rendererConfig.src !== undefined) {
      if (renderer === "rive") patch.rive_src = rendererConfig.src;
      else patch.gif_src = rendererConfig.src;
    }
    if (
      "borderRadius" in rendererConfig &&
      rendererConfig.borderRadius !== undefined
    )
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

export async function dispatchProviderSaved(provider: ProviderInput) {
  await invoke("save_provider", { provider });
  await useProviderStore.getState().loadProviders();
  await emitSettingEventToPeers(SettingEvent.ProviderChanged, {
    action: SettingAction.ProviderSaved,
    id: provider.id,
    value: provider,
  });
}

export async function dispatchProviderDeleted(id: string) {
  await invoke("delete_provider", { id });
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
  await invoke("emit_theme_changed", { resolved });
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
    await invoke("patch_conf", { group, patch });
    const store = useSettingsStore.getState();
    if (group === "app") await store.loadAppConf();
    else if (group === "orb") await store.loadOrbConf();
    else if (group === "mascot") await store.loadMascotConf();
    await emitSettingEventToPeers(SettingEvent.ConfPatched, {
      action: SettingAction.ConfPatched,
      value: { group, patch },
    });
  } catch (e) {
    console.error("[settingsDispatcher] dispatchConfPatch failed:", e);
  }
}
