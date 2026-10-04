import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useOrbStore } from "../stores/orbStore";

const IS_MACOS = /\bMacintosh\b/i.test(navigator.userAgent);

interface UseOrbExpandReturn {
  isExpanded: boolean;
  expand: () => Promise<void>;
  collapse: () => Promise<void>;
}

export function useOrbExpand(): UseOrbExpandReturn {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const storeExpand = useOrbStore((s) => s.expand);
  const storeCollapse = useOrbStore((s) => s.collapse);

  const expand = useCallback(async () => {
    await storeExpand();
    if (IS_MACOS) {
      await invoke("set_orb_click_through", { enabled: false });
    }
  }, [storeExpand]);

  const collapse = useCallback(async () => {
    await storeCollapse();
    if (IS_MACOS) {
      await invoke("set_orb_click_through", { enabled: true });
    }
  }, [storeCollapse]);

  return { isExpanded, expand, collapse };
}
