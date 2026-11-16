import { useEffect } from "react";
import { notification } from "antd";
import { listen } from "@tauri-apps/api/event";
import { UiEvent, type ShortcutWarningPayload } from "@/types/events";
import { translateKey } from "@/i18n";
import { useNotifyStore } from "@/stores/system/notifyStore";

export function useErrorNotifications() {
  const [api, contextHolder] = notification.useNotification();
  const notifyPending = useNotifyStore((s) => s.pending);
  const notifyDequeue = useNotifyStore((s) => s.dequeue);

  useEffect(() => {
    while (notifyPending.length > 0) {
      const item = notifyDequeue();
      if (!item) break;
      const method =
        item.level === "warning"
          ? api.warning
          : item.level === "info"
            ? api.info
            : api.error;
      method({
        message: translateKey(item.titleKey, item.options),
        description:
          item.descriptionText ??
          translateKey(item.descriptionKey, item.options),
        placement: "topRight",
      });
    }
  }, [notifyPending, notifyDequeue, api]);

  useEffect(() => {
    const unlisten = listen<ShortcutWarningPayload>(
      UiEvent.ShortcutWarning,
      ({ payload }) => {
        api.warning({
          message: translateKey("common.window.shortcut_register_failed"),
          description: translateKey("common.window.shortcut_occupied", {
            id: payload.id,
            binding: payload.binding,
          }),
          placement: "topRight",
        });
      },
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [api]);

  return { contextHolder };
}
