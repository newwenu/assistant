import { useEffect } from "react";
import { useToolStore } from "@/stores/toolStore";
import { useSettingsStore } from "@/stores/settingsStore";

export function useAppInitialization() {
  const loadFavorites = useToolStore((s) => s.loadFavorites);
  const loadRecentTools = useToolStore((s) => s.loadRecentTools);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);

  useEffect(() => {
    void loadFavorites();
    void loadRecentTools();
    void loadAppConf();
    void loadOrbConf();
    void loadMascotConf();
  }, [
    loadFavorites,
    loadRecentTools,
    loadAppConf,
    loadOrbConf,
    loadMascotConf,
  ]);
}
