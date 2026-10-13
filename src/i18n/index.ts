import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import zhCN from "./locales/zh-CN.json";
import enUS from "./locales/en-US.json";

export const SUPPORTED_LOCALES = ["zh-CN", "en-US"] as const;
export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];
export const DEFAULT_LOCALE: SupportedLocale = "zh-CN";

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      "zh-CN": { translation: zhCN },
      "en-US": { translation: enUS },
    },
    fallbackLng: DEFAULT_LOCALE,
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ["localStorage", "navigator"],
      lookupLocalStorage: "i18nextLng",
      caches: ["localStorage"],
    },
    missingKeyHandler: (lngs, ns, key) => {
      console.warn(
        `[i18n] missing key: "${key}" (langs: ${lngs.join(", ")}, ns: ${ns})`,
      );
    },
  });

export default i18n;

export type I18nKey = keyof typeof zhCN extends string
  ? keyof typeof zhCN
  : never;

type DeepKeys<T> = T extends object
  ? {
      [K in keyof T & string]: T[K] extends string
        ? K
        : `${K}.${DeepKeys<T[K]>}`;
    }[keyof T & string]
  : never;

export type I18nPath = DeepKeys<typeof zhCN>;

export function translateKey(
  key: string,
  options?: Record<string, unknown>,
): string {
  return i18n.t(key, options as never) as unknown as string;
}
