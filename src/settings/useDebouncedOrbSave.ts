import { useRef, useCallback } from "react";
import type { OrbConf } from "@/stores/settingsStore";
import { dispatchOrbConfChanged } from "@/settings/settingsDispatcher";

const DEBOUNCE_MS = 400;

export function useDebouncedOrbSave(
  orbConf: OrbConf | null,
  onSuccess: () => void,
) {
  const timerRef = useRef<number | null>(null);
  const pendingRef = useRef<OrbConf | null>(null);

  const flush = useCallback(async () => {
    const conf = pendingRef.current;
    if (!conf) return;
    pendingRef.current = null;
    await dispatchOrbConfChanged(conf as unknown as Record<string, unknown>);
    onSuccess();
  }, [onSuccess]);

  const handleSave = useCallback(
    async (values: Partial<OrbConf>, debounce: boolean) => {
      if (!orbConf) return;
      const newConf: OrbConf = { ...orbConf, ...values };
      if (!debounce) {
        if (timerRef.current !== null) {
          clearTimeout(timerRef.current);
          timerRef.current = null;
        }
        pendingRef.current = null;
        await dispatchOrbConfChanged(
          newConf as unknown as Record<string, unknown>,
        );
        onSuccess();
        return;
      }
      pendingRef.current = newConf;
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
