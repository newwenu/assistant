import { useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import type { Event } from "@tauri-apps/api/event";
import { useAttachmentStore } from "../stores/attachmentStore";
import {
  createAttachmentFromPath,
  notifyAttachError,
  ALLOWED_IMAGE_EXTENSIONS,
} from "../chat/core/attachmentUtils";
import { ATTACHMENT_MAX_IMAGES } from "../chat/core/attachmentTypes";

export function useNativeFileAttach(supportVision: boolean) {
  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);

  const addImageFromPath = useCallback(
    (filePath: string) => {
      if (attachments.length >= ATTACHMENT_MAX_IMAGES) {
        notifyAttachError("too_many");
        return;
      }
      const att = createAttachmentFromPath(filePath);
      if (att) {
        addAttachment(att);
      } else {
        notifyAttachError("invalid_type");
      }
    },
    [attachments.length, addAttachment],
  );

  const handleFileSelect = useCallback(async () => {
    if (!supportVision) {
      notifyAttachError("no_vision");
      return;
    }
    const selected = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ALLOWED_IMAGE_EXTENSIONS }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    for (const p of paths) {
      addImageFromPath(p);
    }
  }, [supportVision, addImageFromPath]);

  useEffect(() => {
    if (!supportVision) return;
    const webview = getCurrentWebview();
    let disposed = false;
    const setup = async () => {
      const unlisten = await webview.onDragDropEvent(
        (event: Event<DragDropEvent>) => {
          if (disposed) return;
          if (event.payload.type === "drop") {
            for (const p of event.payload.paths) {
              addImageFromPath(p);
            }
          }
        },
      );
      return unlisten;
    };
    let unlistenFn: (() => void) | undefined;
    void setup().then((fn) => {
      unlistenFn = fn;
    });
    return () => {
      disposed = true;
      unlistenFn?.();
    };
  }, [supportVision, addImageFromPath]);

  return { handleFileSelect };
}
