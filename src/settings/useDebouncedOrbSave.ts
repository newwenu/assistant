import { useRef, useCallback } from "react";
import type { OrbConf } from "@/stores/settingsStore";
import { dispatchConfPatch } from "@/settings/settingsDispatcher";

const DEBOUNCE_MS = 400;

export function useDebouncedOrbSave(
  orbConf: OrbConf | null,
  onSuccess: () => void,
) {
  const timerRef = useRef<number | null>(null);
  const pendingRef = useRef<Partial<OrbConf> | null>(null);

  const flush = useCallback(async () => {
    const patch = pendingRef.current;
    if (!patch) return;
    pendingRef.current = null;
    await dispatchConfPatch("orb", patch as Record<string, unknown>);
    onSuccess();
  }, [onSuccess]);

  const handleSave = useCallback(
    async (values: Partial<OrbConf>, debounce: boolean) => {
      if (!orbConf) return;
      if (!debounce) {
        if (timerRef.current !== null) {
          clearTimeout(timerRef.current);
          timerRef.current = null;
        }
        pendingRef.current = null;
        await dispatchConfPatch("orb", values as Record<string, unknown>);
        onSuccess();
        return;
      }
      pendingRef.current = values;
      if (timerRef.current !== null) clearTimeout(timerRef.current);
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        flush();
      }, DEBOUNCE_MS);
    },
    [orbConf, onSuccess, flush],
  );

  return handleSave;
}