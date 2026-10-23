import React from "react";
import { CloseOutlined } from "@ant-design/icons";
import type { Attachment } from "../core/attachmentTypes";
import css from "./AttachmentPreview.module.css";

interface AttachmentPreviewProps {
  attachments: Attachment[];
  onRemove: (id: string) => void;
}

export const AttachmentPreview: React.FC<AttachmentPreviewProps> = ({
  attachments,
  onRemove,
}) => {
  if (attachments.length === 0) return null;

  return (
    <div className={css.rail}>
      {attachments.map((att) => (
        <div key={att.id} className={css.item}>
          {att.kind === "image" ? (
            <img
              className={css.thumb}
              src={att.previewUrl}
              alt={att.file.name}
            />
          ) : (
            <div className={css.fileIcon}>
              {att.file.name.split(".").pop()?.toUpperCase().slice(0, 4) ??
                "FILE"}
            </div>
          )}
          <button
            type="button"
            className={css.remove}
            onClick={() => onRemove(att.id)}
            aria-label={`Remove ${att.file.name}`}
          >
            <CloseOutlined style={{ fontSize: 10 }} />
          </button>
        </div>
      ))}
    </div>
  );
};
