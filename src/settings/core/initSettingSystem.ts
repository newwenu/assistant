import { registerSettingType } from "./fieldRegistry";
import { registerSideEffect } from "./sideEffectRegistry";
import { AssetPickerField } from "../shared/AssetPickerField";
import { KeybindingField } from "../shared/KeybindingField";
import { dispatchThemeChanged } from "./settingsDispatcher";

export function initSettingSystem(): void {
  registerSettingType("asset-picker", {
    component: AssetPickerField,
  });

  registerSettingType("keybinding", {
    component: KeybindingField,
  });

  registerSideEffect("theme_change", async (_prev, next) => {
    const theme = next.theme as "system" | "light" | "dark";
    await dispatchThemeChanged(theme);
  });

  registerSideEffect("autostart", async (_prev, next) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_auto_launch", {
      enable: !!next.launch_at_login,
    });
  });

  registerSideEffect("always_on_top", async (_prev, next) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_always_on_top", {
      enable: !!next.stay_on_top,
    });
  });

  registerSideEffect("orb_click_through", async (_prev, next) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_click_through", {
      enable: !!next.click_through,
    });
  });

  registerSideEffect("orb_always_on_top", async (_prev, next) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_orb_always_on_top", {
      enable: !!next.always_on_top,
    });
  });
}
