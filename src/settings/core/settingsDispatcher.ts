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
import type { SettingsState } from "@/stores/settingsStore";
import { providerService } from "@/services/providerService";
import { settingService } from "@/services/settingService";
import { createLogger } from "@/services/logger";
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";
import type { ProviderInput } from "@/settings/provider/types";
import type { RendererConfig } from "@/mascot/core/types";
import {
  CONF_KEY_MAP,
  CONF_LOAD_KEY_MAP,
  type ConfGroup,
} from "./field/fieldRegistry";

type ConfUnion = AppConf | OrbConf | MascotConf;

const log = createLogger("settingsDispatcher");

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
  const current = store[confKey];
  if (!current) return;
  useSettingsStore.setState({
    [confKey]: { ...current, [key]: value },
  } as Partial<SettingsState>);
}

export function buildRendererConfig(conf: MascotConf): RendererConfig {
  const renderer = conf.renderer ?? "rive";
  return {
    idlePoseRotation: conf.riveIdlePoseRotation,
    src: renderer === "rive" ? conf.riveSrc : conf.gifSrc,
    animated: conf.svgAnimated,
    borderRadius: conf.gifBorderRadius,
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
      patch.riveIdlePoseRotation = rendererConfig.idlePoseRotation;
    if ("animated" in rendererConfig && rendererConfig.animated !== undefined)
      patch.svgAnimated = rendererConfig.animated;
    if ("src" in rendererConfig && rendererConfig.src !== undefined) {
      if (renderer === "rive") patch.riveSrc = rendererConfig.src;
      else patch.gifSrc = rendererConfig.src;
    }
    if (
      "borderRadius" in rendererConfig &&
      rendererConfig.borderRadius !== undefined
    )
      patch.gifBorderRadius = rendererConfig.borderRadius;
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
        .catch((err: unknown) => log.warn(`emit to ${label} failed`, err));
    }
  }
}

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
  await settingService.emitThemeChanged(resolved);
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
