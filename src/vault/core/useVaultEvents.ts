import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVaultStore } from "./vaultStore";
import { notifyWarning } from "@/stores/notifyStore";

export function useVaultEvents() {
  useEffect(() => {
    let unlistenUnlock: UnlistenFn | undefined;
    let unlistenLock: UnlistenFn | undefined;
    let unlistenMetaWarning: UnlistenFn | undefined;

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

    void listen("vault:meta-integrity-warning", () => {
      notifyWarning(
        "vault.meta.integrity_warning_title",
        "vault.meta.integrity_warning_desc",
      );
    }).then((fn) => {
      unlistenMetaWarning = fn;
    });

    return () => {
      unlistenUnlock?.();
      unlistenLock?.();
      unlistenMetaWarning?.();
    };
  }, []);
}
