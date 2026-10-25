import { useCallback } from "react";
import { useAttachmentStore } from "../stores/attachmentStore";
import {
  createAttachmentFromFile,
  notifyAttachError,
} from "../chat/core/attachmentUtils";
import {
  ATTACHMENT_MAX_IMAGES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
} from "../chat/core/attachmentTypes";

export function usePasteAttach() {
  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);
  const supportVision = useAttachmentStore((s) => s.supportVision);

  const addImageFiles = useCallback(
    (files: File[]) => {
      if (!supportVision) {
        notifyAttachError("no_vision");
        return;
      }
      for (const file of files) {
        if (attachments.length >= ATTACHMENT_MAX_IMAGES) {
          notifyAttachError("too_many");
          break;
        }
        if (file.size > ATTACHMENT_MAX_IMAGE_SIZE_BYTES) {
          notifyAttachError("too_large");
          continue;
        }
        const att = createAttachmentFromFile(file);
        if (att) {
          addAttachment(att);
        } else {
          notifyAttachError("invalid_type");
        }
      }
    },
    [supportVision, attachments.length, addAttachment],
  );

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      const items = Array.from(e.clipboardData.items);
      const imageFiles: File[] = [];
      for (const item of items) {
        if (item.type.startsWith("image/")) {
          const file = item.getAsFile();
          if (file) imageFiles.push(file);
        }
      }
      if (imageFiles.length > 0) {
        e.preventDefault();
        addImageFiles(imageFiles);
      }
    },
    [addImageFiles],
  );

  return { handlePaste };
}
