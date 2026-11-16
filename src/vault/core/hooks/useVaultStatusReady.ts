import { useEffect, useState } from "react";
import { useVaultStore } from "../vaultStore";

export function useVaultStatusReady(): boolean {
  const checkStatus = useVaultStore((s) => s.checkStatus);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void checkStatus().then(() => {
      if (!cancelled) setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, [checkStatus]);

  return ready;
}
