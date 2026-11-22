import React from "react";
import ReactDOM from "react-dom/client";
import { ConfigProvider, App as AntApp } from "antd";
import zhCNAntd from "antd/es/locale/zh_CN";
import enUSAntd from "antd/es/locale/en_US";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { RuntimeLoader } from "@rive-app/webgl2";
import { useThemeStore } from "@/stores/system/themeStore";
import { useLocaleStore } from "@/stores/system/localeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTheme } from "@/hooks/useTheme";
import { lightTheme, darkTheme } from "@/assets/styles/theme";
import { WINDOW_ORB } from "@/config/constants";
import { subscribeSettingEvents } from "@/services/settingEvents";
import { initSettingSystem } from "@/settings/core/registry/initSettingSystem";
import { bootstrapLogger } from "@/services/system/logger";
import { registerGlobalErrorHandlers } from "@/services/system/globalErrorHandler";
import { registerPageZoomGuard } from "@/services/system/pageZoomGuard";
import { AppErrorBoundary } from "@/components/AppErrorBoundary";
import { applyLocaleChange } from "@/i18n/localeApplier";
import { loadFeatures } from "@/features/registry";
import { getConfGroups } from "@/settings/core/runtime/confRuntime";
import { getConfValue } from "@/settings/core/runtime/useConfGroup";
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
registerPageZoomGuard();

async function bootstrap(): Promise<void> {
  await loadFeatures();
  const store = useSettingsStore.getState();
  for (const group of getConfGroups()) {
    await store.loadConf(group);
  }

  const lang = getConfValue("app")?.language ?? "zh-CN";
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
