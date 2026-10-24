import React from "react";
import type { FileAttachmentRef } from "../core/contentBlockTypes";
import { formatBytes } from "@/utils/formatBytes";
import css from "./FileCard.module.css";

interface FileCardProps {
  attachment: FileAttachmentRef;
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
