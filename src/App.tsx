import React from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { BrowserRouter } from "react-router-dom";
import { WINDOW_MAIN, WINDOW_ORB } from "@/config/constants";
import { MainLayout } from "@/layouts/MainLayout";
import { OrbWindow } from "@/layouts/OrbWindow";

const viewMap: Record<string, React.FC> = {
  [WINDOW_MAIN]: MainLayout,
  [WINDOW_ORB]: OrbWindow,
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
