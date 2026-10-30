import { create } from "zustand";
import { DEFAULT_KEYBINDINGS } from "@/settings/core/types/shortcutTypes";
import { createLogger } from "@/services/logger";
export type {
  AppConf,
  OrbConf,
  MascotConf,
  VaultConf,
} from "@/settings/core/types/confTypes";
import type {
  AppConf,
  OrbConf,
  MascotConf,
  VaultConf,
} from "@/settings/core/types/confTypes";
import { settingService } from "../services/settingService";
import { applyLocaleChange } from "@/i18n/localeApplier";

const log = createLogger("settingsStore");

export interface SettingsState {
  appConf: AppConf | null;
  orbConf: OrbConf | null;
  mascotConf: MascotConf | null;
  vaultConf: VaultConf | null;

  loadAppConf: () => Promise<void>;
  loadOrbConf: () => Promise<void>;
  loadMascotConf: () => Promise<void>;
  loadVaultConf: () => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  appConf: null,
  orbConf: null,
  mascotConf: null,
  vaultConf: null,

  loadAppConf: async () => {
    try {
      const prevLang = get().appConf?.language;
      const conf = await settingService.getAppConf();
      set({ appConf: conf });
      if (conf.language && conf.language !== prevLang) {
        await applyLocaleChange(conf.language);
      }
    } catch (e) {
      log.error("loadAppConf failed", e);
    }
  },

  loadOrbConf: async () => {
    try {
      const conf = await settingService.getOrbConf();
      set({ orbConf: conf });
    } catch (e) {
      log.error("loadOrbConf failed", e);
    }
  },

  loadMascotConf: async () => {
    try {
      const conf = await settingService.getMascotConf();
      set({ mascotConf: conf });
    } catch (e) {
      log.error("loadMascotConf failed", e);
    }
  },

  loadVaultConf: async () => {
    try {
      const conf = await settingService.getVaultConf();
      set({ vaultConf: conf });
    } catch (e) {
      log.error("loadVaultConf failed", e);
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
