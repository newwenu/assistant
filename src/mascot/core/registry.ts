import type { FC } from "react";
import type {
  MascotRendererId,
  MascotRendererProps,
  RendererMeta,
} from "./types";

interface RendererEntry {
  meta: RendererMeta;
  component: FC<MascotRendererProps>;
  dispose?: () => void;
}

const registry = new Map<string, RendererEntry>();

export function registerRenderer(entry: RendererEntry): void {
  if (registry.has(entry.meta.id as string)) {
    console.warn(
      `[mascot] renderer "${entry.meta.id}" already registered, overwriting`,
    );
  }
  registry.set(entry.meta.id as string, entry);
}

export function getRenderer(id: MascotRendererId): RendererEntry | undefined {
  return registry.get(id as string);
}

export function getAllRendererMetas(): RendererMeta[] {
  return Array.from(registry.values()).map((e) => e.meta);
}

export function unregisterRenderer(id: MascotRendererId): void {
  const entry = registry.get(id as string);
  entry?.dispose?.();
  registry.delete(id as string);
}
