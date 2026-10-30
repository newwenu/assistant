import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVaultStore } from "./vaultStore";

export function useVaultEvents() {
  useEffect(() => {
    let unlistenUnlock: UnlistenFn | undefined;
    let unlistenLock: UnlistenFn | undefined;

    void listen("vault:unlocked", () => {
      useVaultStore.setState({ isLocked: false, loading: false, error: null });
      useVaultStore.getState().resetAutoLockTimer();
    }).then((fn) => {
      unlistenUnlock = fn;
    });

    void listen("vault:locked", () => {
      useVaultStore.setState({
        isLocked: true,
        entries: [],
        selectedEntry: null,
        revealedField: null,
      });
    }).then((fn) => {
      unlistenLock = fn;
    });

    return () => {
      unlistenUnlock?.();
      unlistenLock?.();
    };
  }, []);
}
