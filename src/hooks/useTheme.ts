import { useEffect } from "react";
import { useThemeStore } from "@/stores/themeStore";

export function useTheme() {
  const init = useThemeStore((s) => s.init);

  useEffect(() => {
    init();
  }, [init]);
}
