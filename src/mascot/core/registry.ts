import type { FC } from "react";
import type {
  MascotRendererId,
  MascotRendererProps,
  RendererMeta,
  RendererSettingDef,
} from "./types";
import {
  defineSetting,
  type SettingDefinition,
  type ConfGroup,
} from "@/settings/core/field/fieldRegistry";

interface RendererEntry {
  meta: RendererMeta;
  component: FC<MascotRendererProps>;
  dispose?: () => void;
}

const registry = new Map<string, RendererEntry>();

function registerRendererSettings(
  rendererId: MascotRendererId,
  settings: RendererSettingDef[],
): void {
  const group: ConfGroup = "mascot";
  const section = "orb-ball";
  const visibleForThisRenderer = (conf: Record<string, unknown>) =>
    conf.renderer === (rendererId as string);

  for (const s of settings) {
    const def: SettingDefinition = {
      key: s.key,
      group,
      section,
      type: s.type as SettingDefinition["type"],
      default: s.default,
      label: s.label,
      description: s.description,
      sideEffect: s.sideEffect,
      visible: visibleForThisRenderer,
      min: s.min,
      max: s.max,
      step: s.step,
      options: s.options,
      marks: s.marks,
      extra: s.extra,
      saveMessage: s.saveMessage,
      assetConfig: s.assetConfig,
    };
    defineSetting(def);
  }
}

export function registerRenderer(entry: RendererEntry): void {
  if (registry.has(entry.meta.id)) {
    console.warn(
      `[mascot] renderer "${entry.meta.id}" already registered, overwriting`,
    );
  }
  registry.set(entry.meta.id, entry);
  if (entry.meta.settings?.length) {
    registerRendererSettings(entry.meta.id, entry.meta.settings);
  }
}

export function getRenderer(id: MascotRendererId): RendererEntry | undefined {
  return registry.get(id);
}

export function getAllRendererMetas(): RendererMeta[] {
  return Array.from(registry.values()).map((e) => e.meta);
}

export function unregisterRenderer(id: MascotRendererId): void {
  const entry = registry.get(id);
  entry?.dispose?.();
  registry.delete(id);
}

type MascotCapabilityKey =
  "colorCustomization" | "idleAnimation" | "hasDefaultColors";

export function createMascotCapabilityVisible(
  ...requiredCaps: MascotCapabilityKey[]
): (conf: Record<string, unknown>) => boolean {
  return (conf) => {
    const rendererId = (conf.renderer as string) ?? "rive";
    const meta = getAllRendererMetas().find(
      (m) => m.id === (rendererId as MascotRendererId),
    );
    const caps: Record<MascotCapabilityKey, boolean> = {
      colorCustomization: !!meta?.capabilities?.colorCustomization,
      idleAnimation: !!meta?.capabilities?.idleAnimation,
      hasDefaultColors: !!meta?.defaultColors,
    };
    return requiredCaps.every((c) => caps[c]);
  };
}
