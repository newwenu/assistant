import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVaultStore } from "./vaultStore";

export function useVaultEvents() {
  useEffect(() => {
    let unlistenUnlock: UnlistenFn | undefined;
    let unlistenLock: UnlistenFn | undefined;
    let unlistenHibpProgress: UnlistenFn | undefined;

    void listen("vault:unlocked", () => {
      useVaultStore.setState({ isLocked: false, loading: false, error: null });
      useVaultStore.getState().resetAutoLockTimer(true);
    }).then((fn) => {
      unlistenUnlock = fn;
    });

    void listen("vault:locked", () => {
      useVaultStore.getState().clearAutoLockTimer();
      useVaultStore.setState({
        isLocked: true,
        entries: [],
        totalEntryCount: 0,
        selectedEntry: null,
        searchQuery: "",
        categoryFilter: null,
        tagFilter: null,
        onlyLeaked: false,
        sortBy: "recent",
        hibpLeaks: {},
      });
      useVaultStore.getState().clearRevealedField();
    }).then((fn) => {
      unlistenLock = fn;
    });

    void listen<{ done: number; total: number }>(
      "vault:hibp-progress",
      (event) => {
        useVaultStore.setState({ hibpProgress: event.payload });
      },
    ).then((fn) => {
      unlistenHibpProgress = fn;
    });

    return () => {
      unlistenUnlock?.();
      unlistenLock?.();
      unlistenHibpProgress?.();
    };
  }, []);
}
