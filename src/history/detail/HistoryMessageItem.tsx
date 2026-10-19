import React, { useCallback, useState } from "react";
import { Typography, Tag } from "antd";
import { DeleteOutlined, BranchesOutlined } from "@ant-design/icons";
import type { Message } from "../../chat/core/chatTypes";
import { BranchPicker } from "../../chat/blocks/BranchPicker";
import css from "./HistoryMessageItem.module.css";

const BRANCH_COLORS = [
  "#1677ff",
  "#52c41a",
  "#faad14",
  "#ff4d4f",
  "#722ed1",
  "#13c2c2",
  "#eb2f96",
  "#fa541c",
];

interface HistoryMessageItemProps {
  siblings: Message[];
  activeIndex: number;
  branchLabel: string;
  onDelete: (id: string) => void;
}

export const HistoryMessageItem: React.FC<HistoryMessageItemProps> = React.memo(
  ({ siblings, activeIndex, branchLabel, onDelete }) => {
    const [localIndex, setLocalIndex] = useState(activeIndex);

    const msg = siblings[localIndex] ?? siblings[0];
    const isUser = msg.role === "user";
    const isSystem = msg.role === "system";
    const siblingCount = siblings.length;
    const handleSwitch = useCallback(
      (direction: "prev" | "next") => {
        if (direction === "next" && localIndex < siblingCount - 1) {
          setLocalIndex((i) => i + 1);
        } else if (direction === "prev" && localIndex > 0) {
          setLocalIndex((i) => i - 1);
        }
      },
      [localIndex, siblingCount],
    );

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

    const colorIdx =
      (parseInt(branchLabel.replace("B", ""), 10) - 1) % BRANCH_COLORS.length;
    const branchColor = BRANCH_COLORS[colorIdx >= 0 ? colorIdx : 0];

    const branchBar = (
      <div
        className={[css.branchBar, isUser ? css.branchBarRight : css.branchBarLeft].join(" ")}
        style={{ backgroundColor: branchColor }}
      />
    );

    return (
      <div className={rootClass}>
        {!isUser && branchBar}
        <div className={css.cardStack}>
          <div className={bubbleClass}>
            <div className={css.meta}>
              <Tag
                color={isUser ? "blue" : isSystem ? "orange" : "default"}
                className={css.roleTag}
              >
                {msg.role}
              </Tag>
              <Tag
                className={css.branchTag}
                icon={<BranchesOutlined />}
                style={{
                  color: branchColor,
                  borderColor: branchColor,
                  background: `${branchColor}10`,
                }}
              >
                {branchLabel}
              </Tag>
              <Typography.Text type="secondary" className={css.metaText}>
                {new Date(msg.timestamp).toLocaleString()}
              </Typography.Text>
              {msg.tokens != null && (
                <Typography.Text type="secondary" className={css.metaText}>
                  {msg.tokens} tokens
                </Typography.Text>
              )}
              <DeleteOutlined
                className={css.deleteIcon}
                onClick={() => onDelete(msg.id)}
              />
            </div>
            <div className={css.content}>{msg.content}</div>
            <BranchPicker
              currentSiblingIndex={localIndex}
              siblingCount={siblingCount}
              onSwitch={handleSwitch}
            />
          </div>
        </div>
        {isUser && branchBar}
      </div>
    );
  },
);