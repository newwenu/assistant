import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface AppConf {
  theme: string;
  language: string;
  hotkey: string;
  stay_on_top: boolean;
  launch_at_login: boolean;
  close_to_tray: boolean;
  keybindings: Record<string, string>;
}

export interface OrbConf {
  click_through: boolean;
  always_on_top: boolean;
  scale_percent: number;
  opacity_percent: number;
  hide_on_pointer_hover: boolean;
  hover_delay_ms: number;
  corner_radius_percent: number;
  position_x: number | null;
  position_y: number | null;
}

interface SettingsState {
  appConf: AppConf | null;
  orbConf: OrbConf | null;

  loadAppConf: () => Promise<void>;
  saveAppConf: (conf: AppConf) => Promise<void>;
  loadOrbConf: () => Promise<void>;
  saveOrbConf: (conf: OrbConf) => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}

const DEFAULT_KEYBINDINGS: Record<string, string> = {
  toggle_orb: "Super+Shift+O",
  toggle_main: "Super+Shift+M",
  new_chat: "Super+Shift+N",
};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  appConf: null,
  orbConf: null,

  loadAppConf: async () => {
    try {
      const conf = (await invoke("get_app_conf")) as AppConf;
      set({ appConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadAppConf failed:", e);
    }
  },

  saveAppConf: async (conf: AppConf) => {
    try {
      await invoke("set_app_conf", { conf });
      set({ appConf: conf });
    } catch (e) {
      console.error("[settingsStore] saveAppConf failed:", e);
    }
  },

  loadOrbConf: async () => {
    try {
      const conf = (await invoke("get_orb_conf")) as OrbConf;
      set({ orbConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadOrbConf failed:", e);
    }
  },

  saveOrbConf: async (conf: OrbConf) => {
    try {
      await invoke("set_orb_conf", { conf });
      set({ orbConf: conf });
    } catch (e) {
      console.error("[settingsStore] saveOrbConf failed:", e);
    }
  },

  updateKeybinding: async (id: string, binding: string) => {
    const { appConf } = get();
    if (!appConf) return;
    const newKeybindings = { ...appConf.keybindings, [id]: binding };
    const newConf = { ...appConf, keybindings: newKeybindings };
    await get().saveAppConf(newConf);
    try {
      await invoke("reload_shortcuts");
    } catch (e) {
      console.error("[settingsStore] reload_shortcuts failed:", e);
    }
  },

  resetKeybindings: async () => {
    const { appConf } = get();
    if (!appConf) return;
    const newConf = { ...appConf, keybindings: { ...DEFAULT_KEYBINDINGS } };
    await get().saveAppConf(newConf);
    try {
      await invoke("reload_shortcuts");
    } catch (e) {
      console.error("[settingsStore] reload_shortcuts failed:", e);
    }
  },
}));
