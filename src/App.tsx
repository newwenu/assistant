import React from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { BrowserRouter } from "react-router-dom";
import {
  WINDOW_MAIN,
  WINDOW_ORB,
  WINDOW_SETTINGS,
  WINDOW_VAULT_UNLOCK,
} from "@/config/constants";
import { MainLayout } from "@/layouts/MainLayout";
import { OrbWindow } from "@/layouts/OrbWindow";
import { SettingsWindow } from "@/layouts/SettingsWindow";
import { VaultUnlockWindow } from "@/layouts/VaultUnlockWindow";

const viewMap: Record<string, React.FC> = {
  [WINDOW_MAIN]: MainLayout,
  [WINDOW_ORB]: OrbWindow,
  [WINDOW_SETTINGS]: SettingsWindow,
  [WINDOW_VAULT_UNLOCK]: VaultUnlockWindow,
};

function App() {
  const webview = getCurrentWebview();
  const View = viewMap[webview.label] ?? MainLayout;
  return (
    <BrowserRouter>
      <View />
    </BrowserRouter>
  );
}

export default App;
