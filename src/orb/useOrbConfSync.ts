import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { OrbConf } from "@/stores/settingsStore";
import { SettingEvent, SettingAction } from "../types/events";
import type { SettingEventPayload } from "../types/events";

async function reloadAndApply(appliedRef: React.RefObject<string>) {
  try {
    const conf = await invoke<OrbConf>("get_orb_conf");
    await applyOrbConf(conf, appliedRef);
  } catch (e) {
    console.warn("[useOrbConfSync] reload failed:", e);
  }
}

async function applyOrbConf(
  conf: OrbConf,
  appliedRef: React.RefObject<string>,
) {
  const key = JSON.stringify(conf);
  if (key === appliedRef.current) return;
  appliedRef.current = key;

  document.documentElement.style.setProperty(
    "--orb-opacity",
    `${conf.opacity_percent / 100}`,
  );
  document.documentElement.style.setProperty(
    "--orb-scale",
    `${conf.scale_percent / 100}`,
  );

  if (conf.hide_on_pointer_hover) {
    document.documentElement.classList.add("orb-hover-hide");
  } else {
    document.documentElement.classList.remove("orb-hover-hide");
  }

  document.documentElement.style.setProperty(
    "--orb-hover-delay",
    `${conf.hover_delay_ms}ms`,
  );
  document.documentElement.style.setProperty(
    "--orb-idle-opacity",
    `${conf.idle_opacity_percent / 100}`,
  );

  try {
    await invoke("set_orb_click_through", { enabled: conf.click_through });
  } catch (e) {
    console.warn("[useOrbConfSync] set_orb_click_through failed:", e);
  }
}

export function useOrbConfSync(): { ready: Promise<void> } {
  const appliedRef = useRef<string>("");
  const readyRef = useRef<Promise<void> | null>(null);

  useEffect(() => {
    readyRef.current = reloadAndApply(appliedRef);

    const unlisten = listen<SettingEventPayload>(
      SettingEvent.ConfPatched,
      ({ payload }) => {
        if (payload.action !== SettingAction.ConfPatched) return;
        const { group } = (payload.value as { group: string }) ?? {};
        if (group === "orb") void reloadAndApply(appliedRef);
      },
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  return { ready: readyRef.current ?? Promise.resolve() };
}
