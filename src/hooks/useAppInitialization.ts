import { useEffect } from "react";
import { useToolStore } from "@/stores/toolStore";

export function useAppInitialization() {
  const loadFavorites = useToolStore((s) => s.loadFavorites);
  const loadRecentTools = useToolStore((s) => s.loadRecentTools);

  useEffect(() => {
    void loadFavorites();
    void loadRecentTools();
  }, [loadFavorites, loadRecentTools]);
}
