import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_KEYBINDINGS } from "@/settings/core/shortcutTypes";

export interface AppConf {
  theme: string;
  language: string;
  hotkey: string;
  stay_on_top: boolean;
  launch_at_login: boolean;
  close_to_tray: boolean;
  keybindings: Record<string, string>;
  auto_compact: boolean;
  compact_threshold_percent: number;
  compact_summary_prompt: string | null;
  compact_summary_max_tokens: number;
}

export interface OrbConf {
  click_through: boolean;
  always_on_top: boolean;
  scale_percent: number;
  opacity_percent: number;
  hide_on_pointer_hover: boolean;
  hover_delay_ms: number;
  idle_opacity_percent: number;
  position_x: number | null;
  position_y: number | null;
}

export interface MascotConf {
  renderer: string;
  primary_color: string | null;
  secondary_color: string | null;
  rive_idle_pose_rotation: boolean;
  rive_src: string | null;
  svg_animated: boolean;
  gif_src: string | null;
  gif_border_radius: number;
}

interface SettingsState {
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
      const conf = await invoke<AppConf>("get_app_conf");
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
      const conf = await invoke<OrbConf>("get_orb_conf");
      set({ orbConf: conf });
    } catch (e) {
      console.error("[settingsStore] loadOrbConf failed:", e);
    }
  },

  loadMascotConf: async () => {
    try {
      const conf = await invoke<MascotConf>("get_mascot_conf");
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
