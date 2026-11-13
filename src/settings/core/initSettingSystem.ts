import { registerSettingType } from "./field/fieldRegistry";
import { registerSideEffect } from "./sideEffectRegistry";
import { registerConfSlot } from "./confRuntime";
import { getBuiltinConfSlots } from "./builtinRegistry";
import { AssetPickerField } from "../shared/renderers/AssetPickerField";
import { KeybindingField } from "../shared/renderers/KeybindingField";
import { dispatchThemeChanged } from "./settingsDispatcher";
import { applyLocaleChange } from "@/i18n/localeApplier";

export function initSettingSystem(): void {
  for (const slot of getBuiltinConfSlots()) {
    registerConfSlot(slot);
  }

  registerSettingType("asset-picker", {
    component: AssetPickerField,
  });

  registerSettingType("keybinding", {
    component: KeybindingField,
  });

  registerSideEffect("theme_change", async (_prev, next) => {
    const theme = next.theme;
    if (theme !== "system" && theme !== "light" && theme !== "dark") return;
    await dispatchThemeChanged(theme);
  });

  registerSideEffect("locale_change", async (_prev, next) => {
    const language = next.language;
    if (typeof language !== "string" || !language) return;
    await applyLocaleChange(language);
  });
}
