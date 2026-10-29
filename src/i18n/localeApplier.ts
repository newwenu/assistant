import i18n from "@/i18n";
import { useLocaleStore } from "@/stores/localeStore";

export async function applyLocaleChange(language: string): Promise<void> {
  if (i18n.language !== language) {
    await i18n.changeLanguage(language);
  }
  localStorage.setItem("i18nextLng", language);
  useLocaleStore.getState().setLocale(language);
}
