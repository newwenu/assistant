import { useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import type { Event } from "@tauri-apps/api/event";
import { useAttachmentStore } from "../stores/attachmentStore";
import {
  createAttachmentFromPath,
  createFileAttachmentFromPath,
  notifyAttachError,
  ALLOWED_IMAGE_EXTENSIONS,
} from "../chat/core/attachmentUtils";
import {
  ALLOWED_FILE_EXTENSIONS,
  ATTACHMENT_MAX_FILES,
  ATTACHMENT_MAX_IMAGES,
} from "../chat/core/attachmentTypes";

export function useNativeFileAttach(supportVision: boolean) {
  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);

  const imageCount = attachments.filter((a) => a.kind === "image").length;
  const fileCount = attachments.filter((a) => a.kind === "file").length;

  const addImageFromPath = useCallback(
    (filePath: string) => {
      if (imageCount >= ATTACHMENT_MAX_IMAGES) {
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
    [imageCount, addAttachment],
  );

  const addFileFromPath = useCallback(
    (filePath: string) => {
      if (fileCount >= ATTACHMENT_MAX_FILES) {
        notifyAttachError("too_many_files");
        return;
      }
      const att = createFileAttachmentFromPath(filePath);
      if (att) {
        addAttachment(att);
      } else {
        notifyAttachError("invalid_file_type");
      }
    },
    [fileCount, addAttachment],
  );

  const addFromPath = useCallback(
    (filePath: string) => {
      const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
      if (ALLOWED_IMAGE_EXTENSIONS.includes(ext)) {
        if (!supportVision) {
          notifyAttachError("no_vision");
          return;
        }
        addImageFromPath(filePath);
      } else {
        addFileFromPath(filePath);
      }
    },
    [supportVision, addImageFromPath, addFileFromPath],
  );

  // 单入口：图片与文档合并在同一个筛选器里，避免用户切换；
  // 无视觉能力时只列文档，选中的路径再按扩展名分流。
  const handleFileSelect = useCallback(async () => {
    const supported = supportVision
      ? [...ALLOWED_IMAGE_EXTENSIONS, ...ALLOWED_FILE_EXTENSIONS]
      : [...ALLOWED_FILE_EXTENSIONS];

    const selected = await open({
      multiple: true,
      filters: [
        { name: "Images & Documents", extensions: [...new Set(supported)] },
      ],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    for (const p of paths) {
      addFromPath(p);
    }
  }, [supportVision, addFromPath]);

  useEffect(() => {
    const webview = getCurrentWebview();
    let disposed = false;
    const setup = async () => {
      const unlisten = await webview.onDragDropEvent(
        (event: Event<DragDropEvent>) => {
          if (disposed) return;
          if (event.payload.type === "drop") {
            for (const p of event.payload.paths) {
              addFromPath(p);
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
  }, [addFromPath]);

  return { handleFileSelect };
}
