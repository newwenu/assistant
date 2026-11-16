import { useSettingsStore } from "@/stores/settingsStore";
import type { ConfGroup, ConfOf } from "../types/confRegistry";

export function getConfValue<G extends ConfGroup>(
  group: G,
): ConfOf<G> | null | undefined {
  return useSettingsStore.getState().confs[group];
}

export function useConfGroup<G extends ConfGroup>(group: G): ConfOf<G> | null {
  return useSettingsStore((s) => s.confs[group] ?? null);
}
