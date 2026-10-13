import React from "react";
import ReactDOM from "react-dom/client";
import { ConfigProvider, App as AntApp } from "antd";
import zhCNAntd from "antd/es/locale/zh_CN";
import enUSAntd from "antd/es/locale/en_US";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useThemeStore } from "@/stores/themeStore";
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

const ThemedApp: React.FC = () => {
  useTheme();
  const resolved = useThemeStore((s) => s.resolved);
  const antdTheme = resolved === "dark" ? darkTheme : lightTheme;
  const isOrb = getCurrentWebview().label === WINDOW_ORB;
  const [antdLocale, setAntdLocale] = React.useState(zhCNAntd);

  React.useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  React.useEffect(() => {
    const cleanup = subscribeSettingEvents();
    return cleanup;
  }, []);

  React.useEffect(() => {
    const updateLocale = async () => {
      const { useSettingsStore } = await import("@/stores/settingsStore");
      const lang = useSettingsStore.getState().appConf?.language ?? "zh-CN";
      setAntdLocale(ANTD_LOCALE_MAP[lang] ?? zhCNAntd);
      const i18n = (await import("@/i18n")).default;
      if (i18n.language !== lang) {
        await i18n.changeLanguage(lang);
      }
    };
    void updateLocale();
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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemedApp />
  </React.StrictMode>,
);
