import { useEffect } from "react";
import { useThemeStore } from "@/stores/system/themeStore";

export function useTheme() {
  const subscribe = useThemeStore((s) => s.subscribe);

  useEffect(() => {
    void subscribe();
  }, [subscribe]);
}
