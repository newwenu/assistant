import { useCallback, useEffect } from "react";
import type { ClipboardEvent } from "react";
import {
  getVisionSupport,
  useAttachmentStore,
} from "../../stores/attachmentStore";
import type { AttachmentResult } from "./factory";
import { createImageAttachmentFromFile, createPathAttachment } from "./factory";
import { notifyAttachError, notifyVisionUnknown } from "./notify";
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
    if (error) {
      notifyAttachError(error);
      return;
    }
    // 视觉能力未知时放行图片，但标注一条说明（与后端「保留图片交服务端判定」一致）
    if (
      result.attachment.kind === "image" &&
      getVisionSupport() === "unknown"
    ) {
      notifyVisionUnknown();
    }
  }, []);

  const ingestPath = useCallback(
    async (filePath: string) => {
      ingest(await createPathAttachment(filePath));
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
    const paths = await pickAttachmentPaths(getVisionSupport());
    for (const path of paths) await ingestPath(path);
  }, [ingestPath]);

  useEffect(
    () =>
      subscribeDragDrop((paths) => {
        for (const path of paths) void ingestPath(path);
      }),
    [ingestPath],
  );

  return { handlePaste, handleFileSelect };
}
