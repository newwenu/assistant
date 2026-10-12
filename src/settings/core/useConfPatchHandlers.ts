import { useCallback } from "react";
import type { ConfGroup } from "./fieldRegistry";
import { dispatchConfPatch, optimisticConfUpdate } from "./settingsDispatcher";

export interface PatchHandlers {
  onPatch: (patch: Record<string, unknown>) => Promise<void>;
  onPatchComplete: (patch: Record<string, unknown>) => Promise<void>;
}

export function useConfPatchHandlers(group: ConfGroup): PatchHandlers {
  const onPatch = useCallback(
    async (patch: Record<string, unknown>) => {
      for (const [key, value] of Object.entries(patch)) {
        optimisticConfUpdate(group, key, value);
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
