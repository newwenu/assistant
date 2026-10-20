import { registerSettingType } from "./field/fieldRegistry";
import { registerSideEffect } from "./sideEffectRegistry";
import { AssetPickerField } from "../shared/renderers/AssetPickerField";
import { KeybindingField } from "../shared/renderers/KeybindingField";
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

  registerSideEffect("locale_change", async (_prev, next) => {
    const language = next.language as string;
    if (!language) return;
    const { applyLocaleChange } = await import("@/i18n/localeApplier");
    await applyLocaleChange(language);
  });
}
