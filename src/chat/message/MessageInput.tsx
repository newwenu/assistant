import React, { useState, useCallback } from "react";
import { Input } from "antd";
import {
  CHAT_INPUT_MIN_ROWS,
  CHAT_INPUT_MAX_ROWS,
} from "../../config/constants";
import { RiveMascot } from "../../mascot/RiveMascot";
import { useMascotStore } from "../../mascot/mascotStore";

interface MessageInputProps {
  onSend: (content: string) => void;
  onCancel: () => void;
  isStreaming: boolean;
  disabled: boolean;
}

export const MessageInput: React.FC<MessageInputProps> = ({
  onSend,
  onCancel,
  isStreaming,
  disabled,
}) => {
  const [value, setValue] = useState("");
  const mascotFace = useMascotStore((s) => s.face);

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
          disabled
            ? "请先配置 AI Provider"
            : isStreaming
              ? "输入下一条消息，回车将在回复结束后发送..."
              : "输入消息..."
        }
        disabled={disabled}
      />
      <button
        type="button"
        className={`chat-mascot-btn${canSend ? " chat-mascot-btn-active" : ""}${isStreaming ? " chat-mascot-btn-streaming" : ""}`}
        onClick={handleMascotClick}
        disabled={!isStreaming && !canSend}
        title={isStreaming ? "停止生成" : "发送消息"}
      >
        <div className="chat-mascot-btn-canvas">
          <RiveMascot face={mascotFace} size={44} />
        </div>
      </button>
    </div>
  );
};
