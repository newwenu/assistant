import React from "react";
import { useTranslation } from "react-i18next";
import { Typography, Tag, Popconfirm } from "antd";
import { DeleteOutlined } from "@ant-design/icons";
import type { Message } from "../../chat/chatTypes";
import css from "./HistoryMessageItem.module.css";

interface HistoryMessageItemProps {
  message: Message;
  onDelete: (id: string) => void;
}

export const HistoryMessageItem: React.FC<HistoryMessageItemProps> = React.memo(
  ({ message: msg, onDelete }) => {
    const { t } = useTranslation();
    const isUser = msg.role === "user";
    const isSystem = msg.role === "system";

    const rootClass = [
      css.root,
      isUser ? "" : isSystem ? css.rootRoleSystem : css.rootRoleAssistant,
    ]
      .filter(Boolean)
      .join(" ");

    const bubbleClass = [
      css.bubble,
      isUser
        ? css.bubbleUser
        : isSystem
          ? css.bubbleSystem
          : css.bubbleAssistant,
    ]
      .filter(Boolean)
      .join(" ");

    return (
      <div className={rootClass}>
        <div className={bubbleClass}>
          <div className={css.meta}>
            <Tag
              color={isUser ? "blue" : isSystem ? "orange" : "default"}
              className={css.roleTag}
            >
              {msg.role}
            </Tag>
            <Typography.Text type="secondary" className={css.metaText}>
              {new Date(msg.timestamp).toLocaleString()}
            </Typography.Text>
            {msg.tokens != null && (
              <Typography.Text type="secondary" className={css.metaText}>
                {msg.tokens} tokens
              </Typography.Text>
            )}
            <Popconfirm
              title={t("chat.message.confirm_delete_message")}
              onConfirm={() => onDelete(msg.id)}
            >
              <DeleteOutlined className={css.deleteIcon} />
            </Popconfirm>
          </div>
          <div className={css.content}>{msg.content}</div>
        </div>
      </div>
    );
  },
);
