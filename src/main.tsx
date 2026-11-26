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
import { WINDOW_MAIN, WINDOW_ORB } from "@/config/constants";
import { subscribeSettingEvents } from "@/services/settingEvents";
import { subscribeStartupCapabilities } from "@/stores/system/startupStore";
import { initSettingSystem } from "@/settings/core/registry/initSettingSystem";
import { bootstrapLogger } from "@/services/system/logger";
import { registerGlobalErrorHandlers } from "@/services/system/globalErrorHandler";
import { registerPageZoomGuard } from "@/services/system/pageZoomGuard";
import { AppErrorBoundary } from "@/components/AppErrorBoundary";
import { loadFeatures } from "@/features/registry";
import { tauriInvoke } from "@/services/system/tauriInvoke";
import {
  WINDOW_CONF_REQUIREMENTS,
  WINDOW_LABELS,
  type WindowLabel,
} from "@/settings/core/runtime/windowRequirements.generated";
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

function isWindowLabel(value: string): value is WindowLabel {
  return (WINDOW_LABELS as readonly string[]).includes(value);
}

initSettingSystem();

bootstrapLogger();
registerGlobalErrorHandlers();
registerPageZoomGuard();

async function bootstrap(): Promise<void> {
  const label = getCurrentWebview().label;
  const groups = isWindowLabel(label)
    ? WINDOW_CONF_REQUIREMENTS[label]
    : WINDOW_CONF_REQUIREMENTS.main;
  for (const group of groups) {
    await useSettingsStore.getState().loadConf(group);
  }
  await useThemeStore.getState().initLocal();
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

  React.useEffect(() => subscribeStartupCapabilities(), []);

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

function reportFirstFrame(): void {
  const startupId = window.__STARTUP_ID__;
  if (!startupId) return;
  const frameEpochMs = Math.round(performance.timeOrigin + performance.now());
  tauriInvoke("report_first_frame", { startupId, frameEpochMs }).catch(
    () => undefined,
  );
}

void bootstrap().then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <AppErrorBoundary>
        <ThemedApp />
      </AppErrorBoundary>
    </React.StrictMode>,
  );
  requestAnimationFrame(reportFirstFrame);
  if (getCurrentWebview().label === WINDOW_MAIN) {
    void loadFeatures();
  }
});
