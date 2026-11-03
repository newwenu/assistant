import { useCallback, useEffect } from "react";
import type { ClipboardEvent } from "react";
import { useAttachmentStore } from "../../stores/attachmentStore";
import type { AttachmentResult } from "./factory";
import {
  createFileAttachmentFromPath,
  createImageAttachmentFromFile,
  createImageAttachmentFromPath,
} from "./factory";
import { notifyAttachError } from "./notify";
import { ALLOWED_IMAGE_EXTENSIONS } from "./types";
import { extractPastedImages } from "./sources/paste";
import { pickAttachmentPaths } from "./sources/filePicker";
import { subscribeDragDrop } from "./sources/dragDrop";

export interface AttachmentIntake {
  handlePaste: (e: ClipboardEvent) => void;
  handleFileSelect: () => Promise<void>;
}

export function useAttachmentIntake(): AttachmentIntake {
  const ingest = useCallback((result: AttachmentResult) => {
    if (!result.ok) {
      notifyAttachError(result.error);
      return;
    }
    const error = useAttachmentStore
      .getState()
      .addAttachment(result.attachment);
    if (error) notifyAttachError(error);
  }, []);

  const ingestPath = useCallback(
    (filePath: string) => {
      const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
      ingest(
        ALLOWED_IMAGE_EXTENSIONS.includes(ext)
          ? createImageAttachmentFromPath(filePath)
          : createFileAttachmentFromPath(filePath),
      );
    },
    [ingest],
  );

  const handlePaste = useCallback(
    (e: ClipboardEvent) => {
      const files = extractPastedImages(e);
      if (files.length === 0) return;
      e.preventDefault();
      for (const file of files) ingest(createImageAttachmentFromFile(file));
    },
    [ingest],
  );

  const handleFileSelect = useCallback(async () => {
    const supportVision = useAttachmentStore.getState().supportVision;
    const paths = await pickAttachmentPaths(supportVision);
    for (const path of paths) ingestPath(path);
  }, [ingestPath]);

  useEffect(
    () =>
      subscribeDragDrop((paths) => {
        for (const path of paths) ingestPath(path);
      }),
    [ingestPath],
  );

  return { handlePaste, handleFileSelect };
}
