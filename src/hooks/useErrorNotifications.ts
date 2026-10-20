import { useEffect } from "react";
import { notification } from "antd";
import { listen } from "@tauri-apps/api/event";
import { UiEvent, type ShortcutWarningPayload } from "@/types/events";
import { translateKey } from "@/i18n";
import { useErrorNotifyStore } from "@/stores/errorNotifyStore";

export function useErrorNotifications() {
  const [api, contextHolder] = notification.useNotification();
  const errorNotifyPending = useErrorNotifyStore((s) => s.pending);
  const errorNotifyDequeue = useErrorNotifyStore((s) => s.dequeue);

  useEffect(() => {
    while (errorNotifyPending.length > 0) {
      const item = errorNotifyDequeue();
      if (!item) break;
      api.error({
        message: translateKey(item.titleKey, item.options),
        description: translateKey(item.descriptionKey, item.options),
        placement: "topRight",
      });
    }
  }, [errorNotifyPending, errorNotifyDequeue, api]);

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
