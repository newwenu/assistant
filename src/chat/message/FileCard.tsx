import React, { useCallback } from "react";
import type { FileAttachmentRef } from "../core/message/contentBlockTypes";
import { openFromPanel } from "@/orb/panelActions";
import { formatBytes } from "@/utils/formatBytes";
import css from "./FileCard.module.css";

interface FileCardProps {
  attachment: FileAttachmentRef;
}

export const FileCard: React.FC<FileCardProps> = ({ attachment }) => {
  const ext = attachment.name.split(".").pop()?.toUpperCase().slice(0, 8) ?? "";
  const hasValidId = attachment.attachmentId.length > 0;

  const handleOpen = useCallback(async () => {
    if (!hasValidId) return;
    await openFromPanel({ kind: "attachment", id: attachment.attachmentId });
  }, [attachment.attachmentId, hasValidId]);

  return (
    <div
      className={`${css.card}${hasValidId ? ` ${css.clickable}` : ""}`}
      title={attachment.name}
      role={hasValidId ? "button" : undefined}
      tabIndex={hasValidId ? 0 : undefined}
      onClick={() => {
        void handleOpen();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          void handleOpen();
        }
      }}
    >
      <span className={css.icon}>{ext || "FILE"}</span>
      <span className={css.name}>{attachment.name}</span>
      <span className={css.meta}>{formatBytes(attachment.bytes)}</span>
    </div>
  );
};
