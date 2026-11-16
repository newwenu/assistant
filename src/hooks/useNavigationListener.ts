import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { UiEvent } from "@/types/events";
import type { NavigateFunction } from "react-router-dom";
import { navigateInCurrentMode } from "@/tools/framework/miniNavigate";

export function useNavigationListener(navigate: NavigateFunction) {
  useEffect(() => {
    const unlisten = listen<string>(UiEvent.Navigate, ({ payload }) => {
      void navigateInCurrentMode(navigate, payload).catch(() => {
        void navigate(payload);
      });
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [navigate]);
}
