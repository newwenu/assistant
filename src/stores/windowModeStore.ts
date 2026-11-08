import { create } from "zustand";
import { windowService, type WindowMode } from "@/services/windowService";
import { createLogger } from "@/services/logger";

const log = createLogger("windowModeStore");

interface WindowModeState {
  mode: WindowMode;
  setMode: (mode: WindowMode) => Promise<void>;
  toggleMode: () => Promise<void>;
  applyExternalMode: (mode: WindowMode) => void;
}

export const useWindowModeStore = create<WindowModeState>((set, get) => ({
  mode: "normal",

  setMode: async (mode) => {
    if (get().mode === mode) return;
    try {
      await windowService.setMainWindowMode(mode);
      set({ mode });
    } catch (e) {
      log.error("setMainWindowMode failed", e);
      throw e;
    }
  },

  toggleMode: async () => {
    const next: WindowMode = get().mode === "normal" ? "mini" : "normal";
    await get().setMode(next);
  },

  applyExternalMode: (mode) => {
    if (get().mode !== mode) set({ mode });
  },
}));

export type { WindowMode };
