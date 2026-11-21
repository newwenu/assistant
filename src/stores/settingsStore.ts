import { create } from "zustand";
import { createLogger } from "@/services/system/logger";
import { translateKey } from "@/i18n";
import type { ConfGroup, ConfStore } from "@/settings/core/types/confRegistry";
import { fetchConf, getConfSlot } from "@/settings/core/runtime/confRuntime";
import { applyLocaleChange } from "@/i18n/localeApplier";

export type {
  AppConf,
  OrbConf,
  MascotConf,
} from "@/settings/core/types/confTypes";

const log = createLogger("settingsStore");

export interface SettingsState {
  confs: ConfStore;
  confErrors: Partial<Record<ConfGroup, string>>;

  loadConf: <G extends ConfGroup>(group: G) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  confs: {},
  confErrors: {},

  loadConf: async (group) => {
    if (!getConfSlot(group)) return;
    const prevLang = group === "app" ? get().confs.app?.language : undefined;
    try {
      const conf = await fetchConf(group);
      const next: ConfStore = { ...get().confs };
      next[group] = conf;
      const nextErrors = { ...get().confErrors };
      delete nextErrors[group];
      set({ confs: next, confErrors: nextErrors });
      if (group === "app") {
        const lang = get().confs.app?.language;
        if (lang && lang !== prevLang) {
          await applyLocaleChange(lang);
        }
      }
    } catch (e) {
      log.error(`loadConf(${group}) failed`, e);
      set({
        confErrors: {
          ...get().confErrors,
          [group]: translateKey("settings.load_conf_failed"),
        },
      });
    }
  },
}));
