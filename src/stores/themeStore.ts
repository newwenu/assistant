import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

type ThemeMode = "system" | "light" | "dark";

interface ThemeState {
  mode: ThemeMode;
  resolved: "light" | "dark";
  setMode: (mode: ThemeMode) => void;
  cycleMode: () => void;
  init: () => Promise<void>;
}

const SYSTEM_THEME_CYCLE: ThemeMode[] = ["system", "light", "dark"];

export const useThemeStore = create<ThemeState>((set, get) => ({
  mode: "system",
  resolved: "light",

  setMode: (mode: ThemeMode) => {
    set({ mode });
    invoke("set_theme", { theme: mode }).catch(console.error);
    if (mode !== "system") {
      set({ resolved: mode });
    }
  },

  cycleMode: () => {
    const { mode } = get();
    const next =
      SYSTEM_THEME_CYCLE[
        (SYSTEM_THEME_CYCLE.indexOf(mode) + 1) % SYSTEM_THEME_CYCLE.length
      ];
    get().setMode(next);
  },

  init: async () => {
    try {
      const conf = await invoke<{ theme: string }>("get_app_conf");
      const mode = (
        ["system", "light", "dark"].includes(conf.theme) ? conf.theme : "system"
      ) as ThemeMode;
      set({ mode });

      const win = getCurrentWindow();
      const systemTheme = (await win.theme()) || "light";
      const resolved = mode === "system" ? systemTheme : mode;
      set({ resolved });

      await win.onThemeChanged(({ payload }) => {
        if (get().mode === "system") {
          set({ resolved: payload as "light" | "dark" });
        }
      });
    } catch {
      set({ mode: "system", resolved: "light" });
    }
  },
}));
