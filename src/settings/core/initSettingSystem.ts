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
}
