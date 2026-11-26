import React from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { BrowserRouter } from "react-router-dom";
import { WINDOW_MAIN, WINDOW_ORB } from "@/config/constants";

type ViewLoader = () => Promise<{ default: React.FC }>;

const viewMap: Record<string, ViewLoader> = {
  [WINDOW_MAIN]: () =>
    import("@/layouts/MainLayout").then((m) => ({ default: m.MainLayout })),
  [WINDOW_ORB]: () =>
    import("@/layouts/OrbWindow").then((m) => ({ default: m.OrbWindow })),
};

function App() {
  const label = getCurrentWebview().label;
  const loadView = viewMap[label] ?? viewMap[WINDOW_MAIN];
  const [View, setView] = React.useState<React.FC | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    void loadView().then((m) => {
      if (!cancelled) setView(() => m.default);
    });
    return () => {
      cancelled = true;
    };
  }, [loadView]);

  if (!View) return null;

  return (
    <BrowserRouter>
      <View />
    </BrowserRouter>
  );
}

export default App;
