import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Input, Typography, Dropdown, Checkbox } from "antd";
import {
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  ClockCircleOutlined,
  PushpinOutlined,
  CopyOutlined,
  LockOutlined,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import type { Conversation } from "../../chat/core/chatTypes";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import { formatConversationTitle } from "../../utils/formatConversationTitle";
import css from "./ConversationListItem.module.css";

interface ConversationListItemProps {
  conversation: Conversation;
  selected: boolean;
  multiSelectMode: boolean;
  checked: boolean;
  focused: boolean;
  index: number;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onExport: (id: string, format: string) => void;
  onTogglePin: (id: string) => void;
  onDuplicate: (id: string) => void;
  onToggleCheck: (id: string) => void;
  onSetFocusedIndex: (index: number) => void;
}

export const ConversationListItem: React.FC<ConversationListItemProps> =
  React.memo(
    ({
      conversation,
      selected,
      multiSelectMode,
      checked,
      focused,
      onSelect,
      onDelete,
      onRename,
      onExport,
      onTogglePin,
      onDuplicate,
      onToggleCheck,
      onSetFocusedIndex,
      index,
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
          label: conversation.isPinned
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
          label: t("history.export_markdown"),
        },
        {
          key: "export-json",
          icon: <ExportOutlined />,
          label: t("history.export_json"),
        },
        {
          key: "export-jsonl",
          icon: <ExportOutlined />,
          label: t("history.export_jsonl"),
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
        else if (key === "export-jsonl") onExport(conversation.id, "jsonl");
        else if (key === "delete") onDelete(conversation.id);
      };

      const rootClass = [
        css.root,
        selected ? css.rootSelected : "",
        checked ? css.rootChecked : "",
        focused ? css.rootFocused : "",
      ]
        .filter(Boolean)
        .join(" ");

      const handleClick = () => {
        onSetFocusedIndex(index);
        if (multiSelectMode) {
          onToggleCheck(conversation.id);
        } else {
          onSelect(conversation.id);
        }
      };

      return (
        <Dropdown
          menu={{ items: menuItems, onClick: handleMenuClick }}
          trigger={multiSelectMode ? [] : ["contextMenu"]}
          disabled={multiSelectMode}
        >
          <div className={rootClass} onClick={handleClick}>
            {multiSelectMode && (
              <Checkbox
                checked={checked}
                className={css.checkbox}
                onClick={(e) => e.stopPropagation()}
                onChange={() => onToggleCheck(conversation.id)}
              />
            )}
            <div className={css.contentBody}>
              {editing && !multiSelectMode ? (
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
                    {conversation.isPinned && (
                      <PushpinOutlined className={css.pinIcon} />
                    )}
                    {conversation.isFull && (
                      <LockOutlined
                        className={css.pinIcon}
                        style={{ color: "var(--ant-color-warning)" }}
                      />
                    )}
                    {formatConversationTitle(conversation) ||
                      t("history.new_conversation")}
                  </Typography.Text>
                  <Typography.Text
                    type="secondary"
                    className={css.time}
                    style={{ display: "block" }}
                  >
                    <ClockCircleOutlined className={css.timeIcon} />
                    {formatRelativeTime(conversation.updatedAt)}
                  </Typography.Text>
                </>
              )}
            </div>
          </div>
        </Dropdown>
      );
    },
  );
