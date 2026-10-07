import React, { useState, useCallback, useRef } from "react";
import {
  CopyOutlined,
  CheckOutlined,
  DeleteOutlined,
  EditOutlined,
  RedoOutlined,
} from "@ant-design/icons";
import css from "./MessageActions.module.css";

interface MessageActionsProps {
  role: string;
  isStreaming?: boolean;
  onCopy?: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onRegenerate?: () => void;
}

const COPY_FEEDBACK_MS = 1500;

export const MessageActions: React.FC<MessageActionsProps> = ({
  role,
  isStreaming,
  onCopy,
  onDelete,
  onEdit,
  onRegenerate,
}) => {
  const isUser = role === "user";
  const isAssistant = role === "assistant";
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<number | null>(null);

  const handleCopy = useCallback(() => {
    onCopy?.();
    setCopied(true);
    if (timerRef.current !== null) clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(
      () => setCopied(false),
      COPY_FEEDBACK_MS,
    );
  }, [onCopy]);

  return (
    <div className={css.root}>
      {onCopy && (
        <button
          className={copied ? css.btnCopied : css.btn}
          onClick={handleCopy}
          title={copied ? "已复制" : "复制"}
          type="button"
        >
          {copied ? (
            <CheckOutlined style={{ fontSize: 12 }} />
          ) : (
            <CopyOutlined style={{ fontSize: 12 }} />
          )}
        </button>
      )}
      {isUser && onEdit && !isStreaming && (
        <button className={css.btn} onClick={onEdit} title="编辑" type="button">
          <EditOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {isAssistant && onRegenerate && !isStreaming && (
        <button
          className={css.btn}
          onClick={onRegenerate}
          title="重新生成"
          type="button"
        >
          <RedoOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {onDelete && (
        <button
          className={css.btn}
          onClick={onDelete}
          title="删除"
          type="button"
        >
          <DeleteOutlined style={{ fontSize: 12 }} />
        </button>
      )}
    </div>
  );
};
