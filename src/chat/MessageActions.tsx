import React from "react";
import {
  CopyOutlined,
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

  return (
    <div className={css.root}>
      {onCopy && (
        <button className={css.btn} onClick={onCopy} title="复制" type="button">
          <CopyOutlined style={{ fontSize: 12 }} />
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
