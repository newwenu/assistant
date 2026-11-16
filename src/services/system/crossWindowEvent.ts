import { getCurrentWebview } from "@tauri-apps/api/webview";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { SettingEvent, type SettingEventPayload } from "@/types/events";
import { WINDOW_MAIN, WINDOW_ORB } from "@/config/constants";
import { createLogger } from "@/services/system/logger";

const log = createLogger("crossWindowEvent");

export async function emitSettingEventToPeers(
  event: SettingEvent,
  payload: SettingEventPayload,
) {
  const selfLabel = getCurrentWebview().label;
  const targets = [WINDOW_MAIN, WINDOW_ORB].filter(
    (label) => label !== selfLabel,
  );
  for (const label of targets) {
    const win = await WebviewWindow.getByLabel(label);
    if (win) {
      win
        .emit(event, payload)
        .catch((err: unknown) => log.warn(`emit to ${label} failed`, err));
    }
  }
}
