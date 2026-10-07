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
  loadOrbConf: () => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}

const IS_MAC = /\bMacintosh\b/i.test(navigator.userAgent);
const PRIMARY_MOD = IS_MAC ? "Meta" : "Control";

const DEFAULT_KEYBINDINGS: Record<string, string> = {
  toggle_orb: `${PRIMARY_MOD}+Shift+O`,
  toggle_main: `${PRIMARY_MOD}+Shift+M`,
  new_chat: `${PRIMARY_MOD}+Shift+N`,
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

  loadOrbConf: async () => {
    try {
      const conf = (await invoke("get_orb_conf")) as OrbConf;
      set({ orbConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadOrbConf failed:", e);
    }
  },

  updateKeybinding: async (id: string, binding: string) => {
    const { appConf } = get();
    if (!appConf) return;
    const newKeybindings = { ...appConf.keybindings, [id]: binding };
    const newConf = { ...appConf, keybindings: newKeybindings };
    const { dispatchAppConfChanged } =
      await import("../settings/settingsDispatcher");
    await dispatchAppConfChanged(newConf as unknown as Record<string, unknown>);
  },

  resetKeybindings: async () => {
    const { appConf } = get();
    if (!appConf) return;
    const newConf = { ...appConf, keybindings: { ...DEFAULT_KEYBINDINGS } };
    const { dispatchAppConfChanged } =
      await import("../settings/settingsDispatcher");
    await dispatchAppConfChanged(newConf as unknown as Record<string, unknown>);
  },
}));
