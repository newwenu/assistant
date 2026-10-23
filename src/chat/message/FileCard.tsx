import React from "react";
import type { FileAttachmentRef } from "../core/contentBlockTypes";
import css from "./FileCard.module.css";

interface FileCardProps {
  attachment: FileAttachmentRef;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export const FileCard: React.FC<FileCardProps> = ({ attachment }) => {
  const ext = attachment.name.split(".").pop()?.toUpperCase().slice(0, 8) ?? "";

  return (
    <div className={css.card} title={attachment.name}>
      <span className={css.icon}>{ext || "FILE"}</span>
      <span className={css.name}>{attachment.name}</span>
      <span className={css.meta}>{formatBytes(attachment.bytes)}</span>
    </div>
  );
};
