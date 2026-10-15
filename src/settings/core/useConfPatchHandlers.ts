import { useCallback } from "react";
import type { ConfGroup } from "./field/fieldRegistry";
import {
  dispatchConfPatch,
  optimisticConfUpdate,
  type ConfOf,
} from "./settingsDispatcher";

export interface PatchHandlers<G extends ConfGroup = ConfGroup> {
  onPatch: (patch: Partial<ConfOf<G>>) => Promise<void>;
  onPatchComplete: (patch: Partial<ConfOf<G>>) => Promise<void>;
}

export function useConfPatchHandlers<G extends ConfGroup>(
  group: G,
): PatchHandlers<G> {
  const onPatch = useCallback(
    async (patch: Partial<ConfOf<G>>) => {
      for (const [key, value] of Object.entries(patch)) {
        optimisticConfUpdate(group, key, value);
      }
      await dispatchConfPatch(group, patch);
    },
    [group],
  );

  const onPatchComplete = useCallback(
    async (patch: Partial<ConfOf<G>>) => {
      await dispatchConfPatch(group, patch);
    },
    [group],
  );

  return { onPatch, onPatchComplete };
}
