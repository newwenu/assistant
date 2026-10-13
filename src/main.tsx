import React from "react";
import ReactDOM from "react-dom/client";
import { ConfigProvider, App as AntApp } from "antd";
import zhCNAntd from "antd/es/locale/zh_CN";
import enUSAntd from "antd/es/locale/en_US";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useThemeStore } from "@/stores/themeStore";
import { useLocaleStore } from "@/stores/localeStore";
import { useTheme } from "@/hooks/useTheme";
import { lightTheme, darkTheme } from "@/assets/styles/theme";
import { WINDOW_ORB } from "@/config/constants";
import { subscribeSettingEvents } from "@/services/settingEvents";
import { initSettingSystem } from "@/settings/core/initSettingSystem";
import "@/i18n";
import App from "./App";
import "./assets/styles/fonts.css";
import "./assets/styles/global.css";

const ANTD_LOCALE_MAP: Record<string, typeof zhCNAntd> = {
  "zh-CN": zhCNAntd,
  "en-US": enUSAntd,
};

initSettingSystem();

export async function applyLocaleChange(language: string): Promise<void> {
  const i18n = (await import("@/i18n")).default;
  if (i18n.language !== language) {
    await i18n.changeLanguage(language);
  }
  localStorage.setItem("i18nextLng", language);
  const { useLocaleStore } = await import("@/stores/localeStore");
  useLocaleStore.getState().setLocale(language);
}

async function bootstrap(): Promise<void> {
  const { useSettingsStore } = await import("@/stores/settingsStore");
  const store = useSettingsStore.getState();
  await store.loadAppConf();
  await store.loadOrbConf();
  await store.loadMascotConf();

  const lang = useSettingsStore.getState().appConf?.language ?? "zh-CN";
  await applyLocaleChange(lang);
}

const ThemedApp: React.FC = () => {
  useTheme();
  const resolved = useThemeStore((s) => s.resolved);
  const antdTheme = resolved === "dark" ? darkTheme : lightTheme;
  const isOrb = getCurrentWebview().label === WINDOW_ORB;
  const language = useLocaleStore((s) => s.language);
  const antdLocale = ANTD_LOCALE_MAP[language] ?? zhCNAntd;

  React.useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  React.useEffect(() => {
    const cleanup = subscribeSettingEvents();
    return cleanup;
  }, []);

  if (isOrb) {
    return (
      <ConfigProvider theme={antdTheme} locale={antdLocale}>
        <App />
      </ConfigProvider>
    );
  }

  return (
    <ConfigProvider theme={antdTheme} locale={antdLocale}>
      <AntApp>
        <App />
      </AntApp>
    </ConfigProvider>
  );
};

void bootstrap().then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ThemedApp />
    </React.StrictMode>,
  );
});
