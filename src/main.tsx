import React from "react";
import ReactDOM from "react-dom/client";
import { ConfigProvider, App as AntApp } from "antd";
import zhCNAntd from "antd/es/locale/zh_CN";
import enUSAntd from "antd/es/locale/en_US";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { RuntimeLoader } from "@rive-app/webgl2";
import { useThemeStore } from "@/stores/themeStore";
import { useLocaleStore } from "@/stores/localeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTheme } from "@/hooks/useTheme";
import { lightTheme, darkTheme } from "@/assets/styles/theme";
import { WINDOW_ORB } from "@/config/constants";
import { subscribeSettingEvents } from "@/services/settingEvents";
import { initSettingSystem } from "@/settings/core/initSettingSystem";
import { bootstrapLogger } from "@/services/logger";
import { registerGlobalErrorHandlers } from "@/services/globalErrorHandler";
import { AppErrorBoundary } from "@/components/AppErrorBoundary";
import { applyLocaleChange } from "@/i18n/localeApplier";
import { VAULT_ENABLED } from "@/features/vault";
import { loadFeatures } from "@/features/registry";
import "@/i18n";
import App from "./App";
import "./assets/styles/fonts.css";
import "./assets/styles/global.css";
import "./settings/styles/settings.css";

RuntimeLoader.setWasmUrl("/rive.wasm");
RuntimeLoader.setWasmFallbackUrl("/rive_fallback.wasm");

const ANTD_LOCALE_MAP: Record<string, typeof zhCNAntd> = {
  "zh-CN": zhCNAntd,
  "en-US": enUSAntd,
};

initSettingSystem();

bootstrapLogger();
registerGlobalErrorHandlers();

async function bootstrap(): Promise<void> {
  await loadFeatures();
  const store = useSettingsStore.getState();
  await store.loadAppConf();
  await store.loadOrbConf();
  await store.loadMascotConf();
  if (VAULT_ENABLED) await store.loadVaultConf();

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
        <AntApp component={false}>
          <App />
        </AntApp>
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
      <AppErrorBoundary>
        <ThemedApp />
      </AppErrorBoundary>
    </React.StrictMode>,
  );
});
