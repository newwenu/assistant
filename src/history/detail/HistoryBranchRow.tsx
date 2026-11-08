import React, { useCallback } from "react";
import { Tag } from "antd";
import { AimOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { MessageBubble } from "../../chat/message/MessageBubble";
import { BRANCH_COLORS, type HistoryMessageRow } from "./historyTree";
import css from "./HistoryBranchView.module.css";

const MAX_INDENT = 6;

interface HistoryBranchRowProps {
  row: HistoryMessageRow;
  isFocusLeaf: boolean;
  onDeleteMessage: (id: string) => void;
  onFocusNode: (nodeId: string) => void;
}

const INTERACTIVE_SELECTOR =
  "button, a, input, textarea, select, [role='button']";

export const HistoryBranchRow: React.FC<HistoryBranchRowProps> = React.memo(
  ({ row, isFocusLeaf, onDeleteMessage, onFocusNode }) => {
    const { t } = useTranslation();
    const message = row.node.message;
    const indent = Math.min(row.indent, MAX_INDENT);
    const color = BRANCH_COLORS[row.lane % BRANCH_COLORS.length];
    const isRawTool = message.role === "tool";

    const handleClick = useCallback(
      (event: React.MouseEvent<HTMLDivElement>) => {
        const target = event.target as HTMLElement | null;
        if (target?.closest(INTERACTIVE_SELECTOR)) return;
        onFocusNode(message.id);
      },
      [message.id, onFocusNode],
    );

    const rowClass = [
      css.row,
      row.dimmed ? css.rowDimmed : "",
      row.onFocusPath ? "" : css.rowInteractive,
    ]
      .filter(Boolean)
      .join(" ");

    return (
      <div
        className={rowClass}
        style={{ paddingLeft: indent * 16 }}
        onClick={handleClick}
        data-message-id={message.id}
      >
        <div className={css.gutter}>
          {row.indent > 0 || row.dimmed ? (
            <div className={css.track} style={{ backgroundColor: color }} />
          ) : (
            <div className={css.gutterSpacer} />
          )}
        </div>
        <div className={css.bubbleWrap}>
          {isRawTool ? (
            <div className={css.rawRow}>
              <div className={css.rawHeader}>
                <Tag color="default" style={{ margin: 0 }}>
                  {message.role}
                </Tag>
                <span>{new Date(message.timestamp).toLocaleString()}</span>
                <span>{message.id}</span>
              </div>
              <div>{message.content}</div>
            </div>
          ) : (
            <MessageBubble
              message={message}
              isStreaming={false}
              showBranchPicker={false}
              onDelete={onDeleteMessage}
            />
          )}
          {isFocusLeaf && (
            <span className={css.focusBadge}>
              <AimOutlined /> {t("history.tree.focus_hint")}
            </span>
          )}
        </div>
      </div>
    );
  },
);
