import { useCallback } from "react";
import type { ConfGroup } from "./settingRegistry";
import { dispatchConfPatch } from "./settingsDispatcher";
import { useSettingsStore } from "@/stores/settingsStore";

const CONF_KEY_MAP: Record<ConfGroup, "appConf" | "orbConf" | "mascotConf"> = {
  app: "appConf",
  orb: "orbConf",
  mascot: "mascotConf",
};

export interface PatchHandlers {
  onPatch: (patch: Record<string, unknown>) => void;
  onPatchComplete: (patch: Record<string, unknown>) => void;
}

export function useConfPatchHandlers(group: ConfGroup): PatchHandlers {
  const onPatch = useCallback(
    async (patch: Record<string, unknown>) => {
      const confKey = CONF_KEY_MAP[group];
      const store = useSettingsStore.getState();
      const current = store[confKey] as Record<string, unknown> | null;
      if (current) {
        useSettingsStore.setState({
          [confKey]: { ...current, ...patch },
        });
      }
      await dispatchConfPatch(group, patch);
    },
    [group],
  );

  const onPatchComplete = useCallback(
    async (patch: Record<string, unknown>) => {
      await dispatchConfPatch(group, patch);
    },
    [group],
  );

  return { onPatch, onPatchComplete };
}
