import React from "react";
import ReactDOM from "react-dom/client";
import { ConfigProvider, App as AntApp } from "antd";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useThemeStore } from "@/stores/themeStore";
import { useTheme } from "@/hooks/useTheme";
import { lightTheme, darkTheme } from "@/assets/styles/theme";
import { WINDOW_ORB } from "@/config/constants";
import App from "./App";
import "./assets/styles/global.css";

const ThemedApp: React.FC = () => {
  useTheme();
  const resolved = useThemeStore((s) => s.resolved);
  const antdTheme = resolved === "dark" ? darkTheme : lightTheme;
  const isOrb = getCurrentWebview().label === WINDOW_ORB;

  React.useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  if (isOrb) {
    return (
      <ConfigProvider theme={antdTheme}>
        <App />
      </ConfigProvider>
    );
  }

  return (
    <ConfigProvider theme={antdTheme}>
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