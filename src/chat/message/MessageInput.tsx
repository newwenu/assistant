import React, { useState, useCallback } from "react";
import { Input, Button } from "antd";
import { SendOutlined, CloseCircleOutlined } from "@ant-design/icons";
import {
  CHAT_INPUT_MIN_ROWS,
  CHAT_INPUT_MAX_ROWS,
} from "../../config/constants";

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

  return (
    <div className="chat-input">
      <Input.TextArea
        value={value}
        onChange={(e) => setValue(e.target.value)}
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
      {isStreaming ? (
        <Button
          type="default"
          danger
          icon={<CloseCircleOutlined />}
          onClick={onCancel}
          className="chat-send-btn"
        />
      ) : (
        <Button
          type="primary"
          icon={<SendOutlined />}
          onClick={handleSend}
          disabled={disabled || value.trim().length === 0}
          className="chat-send-btn"
        />
      )}
    </div>
  );
};
