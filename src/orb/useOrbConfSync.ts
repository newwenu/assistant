import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { fetchConf } from "@/settings/core/confRuntime";
import { orbService } from "../services/orbService";
import type { OrbConf } from "@/settings/core/types/confTypes";
import { SettingEvent, SettingAction } from "../types/events";
import type { SettingEventPayload } from "../types/events";
import { createLogger } from "@/services/logger";

const log = createLogger("useOrbConfSync");

async function reloadAndApply(appliedRef: React.RefObject<string>) {
  try {
    const conf = await fetchConf("orb");
    if (!conf) return;
    await applyOrbConf(conf, appliedRef);
  } catch (e) {
    log.warn("reload failed", e);
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
    `${conf.opacityPercent / 100}`,
  );
  document.documentElement.style.setProperty(
    "--orb-scale",
    `${conf.scalePercent / 100}`,
  );

  if (conf.hideOnPointerHover) {
    document.documentElement.classList.add("orb-hover-hide");
  } else {
    document.documentElement.classList.remove("orb-hover-hide");
  }

  document.documentElement.style.setProperty(
    "--orb-hover-delay",
    `${conf.hoverDelayMs}ms`,
  );
  document.documentElement.style.setProperty(
    "--orb-idle-opacity",
    `${conf.idleOpacityPercent / 100}`,
  );

  try {
    await orbService.setOrbClickThrough(conf.clickThrough);
  } catch (e) {
    log.warn("set_orb_click_through failed", e);
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
