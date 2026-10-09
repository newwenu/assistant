import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { OrbConf } from "@/stores/settingsStore";
import { SettingEvent, SettingAction } from "../types/events";
import type { SettingEventPayload } from "../types/events";

async function reloadAndApply(
  appliedRef: React.MutableRefObject<string>,
) {
  try {
    const conf = await invoke<OrbConf>("get_orb_conf");
    await applyOrbConf(conf, appliedRef);
  } catch (e) {
    console.warn("[useOrbConfSync] reload failed:", e);
  }
}

async function applyOrbConf(
  conf: OrbConf,
  appliedRef: React.MutableRefObject<string>,
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
}

export function useOrbConfSync(): void {
  const appliedRef = useRef<string>("");

  useEffect(() => {
    reloadAndApply(appliedRef);

    const unlisten = listen<SettingEventPayload>(
      SettingEvent.ConfPatched,
      ({ payload }) => {
        if (payload.action !== SettingAction.ConfPatched) return;
        const { group } = (payload.value as { group: string }) ?? {};
        if (group === "orb") reloadAndApply(appliedRef);
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}