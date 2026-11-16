import React, { useCallback } from "react";
import { Input } from "antd";
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
import { useConfGroup } from "@/settings/core/runtime/useConfGroup";
import { useAttachmentStore } from "../../stores/attachmentStore";
import {
  buildContentBlocksAsync,
  serializeContent,
} from "../core/message/contentBlockUtils";

interface MessageInputProps {
  /** 受控草稿：由 `ConversationView` 按 `conversationId` 持有，布局替换不丢草稿。 */
  value: string;
  onValueChange: (value: string) => void;
  onSend: (content: string) => void;
  onCancel: () => void;
  onPaste: (e: React.ClipboardEvent) => void;
  isStreaming: boolean;
  disabled: boolean;
  isFull?: boolean;
}

export const MessageInput: React.FC<MessageInputProps> = ({
  value,
  onValueChange,
  onSend,
  onCancel,
  onPaste,
  isStreaming,
  disabled,
  isFull = false,
}) => {
  const mascotFace = useMascotStore((s) => s.face);
  const mascotConf = useConfGroup("mascot");
  const attachments = useAttachmentStore((s) => s.attachments);
  const clearAttachments = useAttachmentStore((s) => s.clearAttachments);
  const { t } = useTranslation();

  const doSend = useCallback(async () => {
    const hasText = value.trim().length > 0;
    const hasAttachments = attachments.length > 0;
    if (!hasText && !hasAttachments) return;
    if (disabled || isStreaming) return;

    const blocks = await buildContentBlocksAsync(value, attachments);
    const content = serializeContent(blocks);
    onSend(content);
    onValueChange("");
    clearAttachments();
  }, [
    value,
    attachments,
    disabled,
    isStreaming,
    onSend,
    onValueChange,
    clearAttachments,
  ]);

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
          onValueChange(e.target.value);
          useMascotStore.getState().notifyListening();
        }}
        onKeyDown={handleKeyDown}
        onPaste={onPaste}
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
            renderer={asRendererId(mascotConf?.renderer ?? "svg")}
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
