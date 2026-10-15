import React, { useState, useCallback } from "react";
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
import { buildRendererConfig } from "../../settings/core/settingsDispatcher";
import { useSettingsStore } from "../../stores/settingsStore";

interface MessageInputProps {
  onSend: (content: string) => void;
  onCancel: () => void;
  isStreaming: boolean;
  disabled: boolean;
  isFull?: boolean;
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
  const { t } = useTranslation();

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey && !disabled && !isStreaming) {
        e.preventDefault();
        if (value.trim().length > 0) {
          onSend(value.trim());
          setValue("");
        }
      }
    },
    [value, disabled, isStreaming, onSend],
  );

  const handleSend = useCallback(() => {
    if (value.trim().length > 0 && !disabled && !isStreaming) {
      onSend(value.trim());
      setValue("");
    }
  }, [value, disabled, isStreaming, onSend]);

  const handleMascotClick = useCallback(() => {
    if (isStreaming) {
      onCancel();
    } else {
      handleSend();
    }
  }, [isStreaming, onCancel, handleSend]);

  const canSend = !disabled && !isStreaming && value.trim().length > 0;

  return (
    <div className="chat-input">
      <Input.TextArea
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          useMascotStore.getState().notifyListening();
        }}
        onKeyDown={handleKeyDown}
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
            primaryColor={hexToArgb(mascotConf?.primary_color)}
            secondaryColor={hexToArgb(mascotConf?.secondary_color)}
            rendererConfig={
              mascotConf ? buildRendererConfig(mascotConf) : undefined
            }
          />
        </div>
      </button>
    </div>
  );
};
