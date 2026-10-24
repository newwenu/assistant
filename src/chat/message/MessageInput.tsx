import React, { useState, useCallback } from "react";
import { Input, message } from "antd";
import { useTranslation } from "react-i18next";
import {
  CHAT_INPUT_MIN_ROWS,
  CHAT_INPUT_MAX_ROWS,
} from "../../config/constants";
import {
  MascotRenderer,
  asRendererId,
  hexToArgb,
  useMascotStore,
} from "../../mascot";
import { buildRendererConfig } from "../../mascot/core/mascotConfig";
import { useSettingsStore } from "../../stores/settingsStore";
import { useAttachmentStore } from "../../stores/attachmentStore";
import {
  buildContentBlocksAsync,
  serializeContent,
  validateImageFile,
} from "../core/contentBlockUtils";
import type { Attachment } from "../core/attachmentTypes";
import {
  ATTACHMENT_MAX_IMAGES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
} from "../core/attachmentTypes";

interface MessageInputProps {
  onSend: (content: string) => void;
  onCancel: () => void;
  isStreaming: boolean;
  disabled: boolean;
  isFull?: boolean;
}

function createImageAttachment(file: File): Attachment | null {
  const validation = validateImageFile(file);
  if (!validation.ok) return null;

  const previewUrl = URL.createObjectURL(file);
  return {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    kind: "image",
    file,
    previewUrl,
    mimeType: file.type as Attachment["mimeType"],
    originalSizeBytes: file.size,
  };
}

export const MessageInput: React.FC<MessageInputProps> = ({
  onSend,
  onCancel,
  isStreaming,
  disabled,
  isFull = false,
}) => {
  const [value, setValue] = useState("");
  const mascotFace = useMascotStore((s) => s.face);
  const mascotConf = useSettingsStore((s) => s.mascotConf);
  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);
  const clearAttachments = useAttachmentStore((s) => s.clearAttachments);
  const supportVision = useAttachmentStore((s) => s.supportVision);
  const { t } = useTranslation();

  const addImageFiles = useCallback(
    (files: File[]) => {
      if (!supportVision) {
        message.info(t("chat.input.model_no_vision"));
        return;
      }
      for (const file of files) {
        if (attachments.length >= ATTACHMENT_MAX_IMAGES) {
          message.warning(
            t("chat.input.too_many_images", { max: ATTACHMENT_MAX_IMAGES }),
          );
          break;
        }
        if (file.size > ATTACHMENT_MAX_IMAGE_SIZE_BYTES) {
          message.warning(
            t("chat.input.image_too_large", {
              max: ATTACHMENT_MAX_IMAGE_SIZE_BYTES / (1024 * 1024),
            }),
          );
          continue;
        }
        const att = createImageAttachment(file);
        if (att) {
          addAttachment(att);
        } else {
          message.warning(t("chat.input.invalid_image_type"));
        }
      }
    },
    [supportVision, attachments.length, addAttachment, t],
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

  const doSend = useCallback(async () => {
    const hasText = value.trim().length > 0;
    const hasAttachments = attachments.length > 0;
    if (!hasText && !hasAttachments) return;
    if (disabled || isStreaming) return;

    const blocks = await buildContentBlocksAsync(value, attachments);
    const content = serializeContent(blocks);
    onSend(content);
    setValue("");
    clearAttachments();
  }, [value, attachments, disabled, isStreaming, onSend, clearAttachments]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey && !disabled && !isStreaming) {
        e.preventDefault();
        void doSend();
      }
    },
    [disabled, isStreaming, doSend],
  );

  const handleMascotClick = useCallback(() => {
    if (isStreaming) {
      onCancel();
    } else {
      void doSend();
    }
  }, [isStreaming, onCancel, doSend]);

  const canSend =
    !disabled &&
    !isStreaming &&
    (value.trim().length > 0 || attachments.length > 0);

  return (
    <div className="chat-input">
      <Input.TextArea
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          useMascotStore.getState().notifyListening();
        }}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
        autoSize={{
          minRows: CHAT_INPUT_MIN_ROWS,
          maxRows: CHAT_INPUT_MAX_ROWS,
        }}
        placeholder={
          isFull
            ? t("chat.input.placeholder_full")
            : disabled
              ? t("chat.input.placeholder_no_provider")
              : isStreaming
                ? t("chat.input.placeholder_streaming")
                : t("chat.input.placeholder")
        }
        disabled={disabled}
      />
      <button
        type="button"
        className={`chat-mascot-btn${canSend ? " chat-mascot-btn-active" : ""}${isStreaming ? " chat-mascot-btn-streaming" : ""}`}
        onClick={handleMascotClick}
        disabled={!isStreaming && !canSend}
        title={isStreaming ? t("chat.input.stop") : t("chat.input.send")}
      >
        <div className="chat-mascot-btn-canvas">
          <MascotRenderer
            renderer={asRendererId(mascotConf?.renderer ?? "rive")}
            face={mascotFace}
            size={44}
            primaryColor={hexToArgb(mascotConf?.primaryColor)}
            secondaryColor={hexToArgb(mascotConf?.secondaryColor)}
            rendererConfig={
              mascotConf ? buildRendererConfig(mascotConf) : undefined
            }
          />
        </div>
      </button>
    </div>
  );
};
