import { useEffect } from "react";
import { useThemeStore } from "@/stores/system/themeStore";

export function useTheme() {
  const init = useThemeStore((s) => s.init);

  useEffect(() => {
    void init();
  }, [init]);
}
