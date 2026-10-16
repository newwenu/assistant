import { create } from "zustand";
import { DEFAULT_KEYBINDINGS } from "@/settings/core/types/shortcutTypes";
export type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";
import { settingService } from "../services/settingService";

export interface SettingsState {
  appConf: AppConf | null;
  orbConf: OrbConf | null;
  mascotConf: MascotConf | null;

  loadAppConf: () => Promise<void>;
  loadOrbConf: () => Promise<void>;
  loadMascotConf: () => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  appConf: null,
  orbConf: null,
  mascotConf: null,

  loadAppConf: async () => {
    try {
      const prevLang = get().appConf?.language;
      const conf = await settingService.getAppConf();
      set({ appConf: conf });
      if (conf.language && conf.language !== prevLang) {
        const { applyLocaleChange } = await import("../main");
        await applyLocaleChange(conf.language);
      }
    } catch (e) {
      console.error("[settingsStore] loadAppConf failed:", e);
    }
  },

  loadOrbConf: async () => {
    try {
      const conf = await settingService.getOrbConf();
      set({ orbConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadOrbConf failed:", e);
    }
  },

  loadMascotConf: async () => {
    try {
      const conf = await settingService.getMascotConf();
      set({ mascotConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadMascotConf failed:", e);
    }
  },

  updateKeybinding: async (id: string, binding: string) => {
    const { appConf } = get();
    if (!appConf) return;
    const newKeybindings = { ...appConf.keybindings, [id]: binding };
    const { dispatchConfPatch } =
      await import("../settings/core/settingsDispatcher");
    await dispatchConfPatch("app", { keybindings: newKeybindings });
  },

  resetKeybindings: async () => {
    const { dispatchConfPatch } =
      await import("../settings/core/settingsDispatcher");
    await dispatchConfPatch("app", { keybindings: { ...DEFAULT_KEYBINDINGS } });
  },
}));
