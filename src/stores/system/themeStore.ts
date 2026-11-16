import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { dispatchThemeChanged } from "../../settings/core/runtime/settingsDispatcher";
import { dispatchConfPatch } from "../../settings/core/runtime/settingsDispatcher";
import { fetchConf } from "../../settings/core/runtime/confRuntime";
import { createLogger } from "@/services/system/logger";
import { UiEvent } from "@/types/events";
import type {
  ResolvedThemeDto,
  ThemeChangedPayloadDto,
  ThemeModeDto,
} from "@/types/protocol.contract";

const log = createLogger("themeStore");

interface ThemeState {
  mode: ThemeModeDto;
  resolved: ResolvedThemeDto;
  setMode: (mode: ThemeModeDto) => void;
  cycleMode: () => void;
  init: () => Promise<void>;
}

const SYSTEM_THEME_CYCLE: ThemeModeDto[] = ["system", "light", "dark"];

function resolveTheme(
  mode: ThemeModeDto,
  systemTheme: ResolvedThemeDto,
): ResolvedThemeDto {
  return mode === "system" ? systemTheme : mode;
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  mode: "system",
  resolved: "light",

  setMode: (mode: ThemeModeDto) => {
    dispatchConfPatch("app", { theme: mode }).catch((e) =>
      log.warn("patchConf for theme failed", e),
    );
    dispatchThemeChanged(mode).catch((e) =>
      log.warn("dispatchThemeChanged failed", e),
    );
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
      const conf = await fetchConf("app");
      const mode = conf?.theme ?? "system";
      set({ mode });

      const win = getCurrentWindow();
      const systemTheme = (await win.theme()) ?? "light";
      set({ resolved: resolveTheme(mode, systemTheme) });

      await win.onThemeChanged(({ payload }) => {
        if (get().mode === "system") {
          set({ resolved: resolveTheme("system", payload) });
        }
      });

      await listen<ThemeChangedPayloadDto>(
        UiEvent.ThemeChanged,
        ({ payload }) => {
          set({ resolved: payload.resolved });
        },
      );
    } catch (e) {
      log.warn("Theme init failed, using defaults", e);
      set({ mode: "system", resolved: "light" });
    }
  },
}));
