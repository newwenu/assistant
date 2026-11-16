import { useEffect } from "react";
import { useVaultStore } from "../vaultStore";

export function useVaultAutoLock() {
  const resetAutoLockTimer = useVaultStore((s) => s.resetAutoLockTimer);

  useEffect(() => {
    const handler = () => {
      if (useVaultStore.getState().isLocked) return;
      resetAutoLockTimer();
    };
    window.addEventListener("mousemove", handler);
    window.addEventListener("keydown", handler);

    const visibilityHandler = () => {
      if (
        document.visibilityState === "visible" &&
        !useVaultStore.getState().isLocked
      ) {
        resetAutoLockTimer();
      }
    };
    document.addEventListener("visibilitychange", visibilityHandler);

    return () => {
      window.removeEventListener("mousemove", handler);
      window.removeEventListener("keydown", handler);
      document.removeEventListener("visibilitychange", visibilityHandler);
    };
  }, [resetAutoLockTimer]);
}
