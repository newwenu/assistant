import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { dispatchThemeChanged } from "../settings/core/settingsDispatcher";

type ThemeMode = "system" | "light" | "dark";

interface ThemeState {
  mode: ThemeMode;
  resolved: "light" | "dark";
  setMode: (mode: ThemeMode) => void;
  cycleMode: () => void;
  init: () => Promise<void>;
}

const SYSTEM_THEME_CYCLE: ThemeMode[] = ["system", "light", "dark"];

const THEME_EVENT = "theme-changed";

function resolveTheme(mode: ThemeMode, systemTheme: string): "light" | "dark" {
  return mode === "system"
    ? (systemTheme as "light" | "dark") || "light"
    : mode;
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  mode: "system",
  resolved: "light",

  setMode: (mode: ThemeMode) => {
    dispatchThemeChanged(mode).catch(console.error);
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
      const resolved = resolveTheme(mode, systemTheme);
      set({ resolved });

      await win.onThemeChanged(({ payload }) => {
        if (get().mode === "system") {
          const r = resolveTheme("system", payload);
          set({ resolved: r });
        }
      });

      await listen<{ resolved: "light" | "dark" }>(
        THEME_EVENT,
        ({ payload }) => {
          set({ resolved: payload.resolved });
        },
      );
    } catch {
      set({ mode: "system", resolved: "light" });
    }
  },
}));
