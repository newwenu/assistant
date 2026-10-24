import React from "react";
import { CheckOutlined, CloseOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { ImageBlock } from "../core/contentBlockTypes";
import { ImageThumbnail } from "../message/ImageThumbnail";
import css from "./EditBlock.module.css";

interface EditBlockProps {
  value: string;
  onChange: (value: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
  imageBlocks?: ImageBlock[];
  onRemoveImage?: (index: number) => void;
}

export const EditBlock: React.FC<EditBlockProps> = ({
  value,
  onChange,
  onConfirm,
  onCancel,
  imageBlocks,
  onRemoveImage,
}) => {
  const { t } = useTranslation();
  const hasImages = imageBlocks && imageBlocks.length > 0;

  return (
    <div className={css.root}>
      {hasImages && (
        <div className={css.imageRail}>
          {imageBlocks.map((block, i) => (
            <div key={block.attachment.attachmentId} className={css.imageItem}>
              <ImageThumbnail
                attachment={block.attachment}
                cachedData={block.cached_data}
                offloaded={block.offloaded}
              />
              {onRemoveImage && (
                <button
                  type="button"
                  className={css.imageRemove}
                  onClick={() => onRemoveImage(i)}
                  aria-label={t("common.actions.delete")}
                >
                  <CloseOutlined style={{ fontSize: 10 }} />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      <textarea
        className={css.textarea}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            onConfirm();
          }
          if (e.key === "Escape") {
            onCancel();
          }
        }}
      />
      <div className={css.actions}>
        <button
          className={css.cancelBtn}
          onClick={onCancel}
          title={t("common.actions.cancel")}
          type="button"
        >
          <CloseOutlined style={{ fontSize: 14 }} />
        </button>
        <button
          className={css.confirmBtn}
          onClick={onConfirm}
          title={t("common.actions.confirm")}
          type="button"
        >
          <CheckOutlined style={{ fontSize: 14 }} />
        </button>
      </div>
    </div>
  );
};
