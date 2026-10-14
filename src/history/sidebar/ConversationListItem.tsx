import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Input, Typography, Dropdown } from "antd";
import {
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  ClockCircleOutlined,
  PushpinOutlined,
  CopyOutlined,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import type { Conversation } from "../../chat/chatTypes";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import css from "./ConversationListItem.module.css";

interface ConversationListItemProps {
  conversation: Conversation;
  selected: boolean;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onExport: (id: string, format: string) => void;
  onTogglePin: (id: string) => void;
  onDuplicate: (id: string) => void;
}

export const ConversationListItem: React.FC<ConversationListItemProps> =
  React.memo(
    ({
      conversation,
      selected,
      onSelect,
      onDelete,
      onRename,
      onExport,
      onTogglePin,
      onDuplicate,
    }) => {
      const { t } = useTranslation();
      const [editing, setEditing] = useState(false);
      const [editTitle, setEditTitle] = useState("");

      const startRename = () => {
        setEditTitle(conversation.title);
        setEditing(true);
      };

      const confirmRename = () => {
        if (editTitle.trim() && editTitle !== conversation.title) {
          onRename(conversation.id, editTitle.trim());
        }
        setEditing(false);
      };

      const menuItems: MenuProps["items"] = [
        {
          key: "rename",
          icon: <EditOutlined />,
          label: t("common.actions.rename"),
        },
        {
          key: "pin",
          icon: <PushpinOutlined />,
          label: conversation.is_pinned
            ? t("common.actions.unpin")
            : t("common.actions.pin"),
        },
        {
          key: "duplicate",
          icon: <CopyOutlined />,
          label: t("common.actions.duplicate"),
        },
        {
          key: "export-md",
          icon: <ExportOutlined />,
          label: t("chat.history.export_markdown"),
        },
        {
          key: "export-json",
          icon: <ExportOutlined />,
          label: t("chat.history.export_json"),
        },
        { type: "divider" },
        {
          key: "delete",
          icon: <DeleteOutlined />,
          label: t("common.actions.delete"),
          danger: true,
        },
      ];

      const handleMenuClick: MenuProps["onClick"] = ({ key }) => {
        if (key === "rename") startRename();
        else if (key === "pin") onTogglePin(conversation.id);
        else if (key === "duplicate") onDuplicate(conversation.id);
        else if (key === "export-md") onExport(conversation.id, "markdown");
        else if (key === "export-json") onExport(conversation.id, "json");
        else if (key === "delete") onDelete(conversation.id);
      };

      const rootClass = [css.root, selected ? css.rootSelected : ""]
        .filter(Boolean)
        .join(" ");

      return (
        <Dropdown
          menu={{ items: menuItems, onClick: handleMenuClick }}
          trigger={["contextMenu"]}
        >
          <div className={rootClass} onClick={() => onSelect(conversation.id)}>
            {editing ? (
              <Input
                size="small"
                value={editTitle}
                onChange={(e) => setEditTitle(e.target.value)}
                onPressEnter={confirmRename}
                onBlur={confirmRename}
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <>
                <Typography.Text
                  ellipsis
                  className={css.title}
                  style={{ display: "block" }}
                >
                  {conversation.is_pinned && (
                    <PushpinOutlined className={css.pinIcon} />
                  )}
                  {conversation.title || t("chat.history.new_conversation")}
                </Typography.Text>
                <Typography.Text
                  type="secondary"
                  className={css.time}
                  style={{ display: "block" }}
                >
                  <ClockCircleOutlined className={css.timeIcon} />
                  {formatRelativeTime(conversation.updated_at)}
                </Typography.Text>
              </>
            )}
          </div>
        </Dropdown>
      );
    },
  );
