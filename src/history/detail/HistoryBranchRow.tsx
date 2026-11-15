import React, { useCallback, useMemo } from "react";
import { Button, Tag, Tooltip } from "antd";
import { AimOutlined, CopyOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { MessageBubble } from "../../chat/message/MessageBubble";
import type { MessageRecordDto } from "../../types/protocol.contract";
import { BRANCH_COLORS, type HistoryMessageRow } from "./historyTree";
import { MessageMetaLine } from "./MessageMetaLine";
import { toRawRecordJson, type RecordSource } from "./recordSnapshot";
import css from "./HistoryBranchView.module.css";

const MAX_INDENT = 6;

export type HistoryViewMode = "read" | "raw";

interface HistoryBranchRowProps {
  row: HistoryMessageRow;
  viewMode: HistoryViewMode;
  record: MessageRecordDto | null;
  recordError: string | null;
  isFocusLeaf: boolean;
  onDeleteMessage: (id: string) => void;
  onFocusNode: (nodeId: string) => void;
}

const INTERACTIVE_SELECTOR =
  "button, a, input, textarea, select, [role='button']";

export const HistoryBranchRow: React.FC<HistoryBranchRowProps> = React.memo(
  ({
    row,
    viewMode,
    record,
    recordError,
    isFocusLeaf,
    onDeleteMessage,
    onFocusNode,
  }) => {
    const { t } = useTranslation();
    const message = row.node.message;
    const indent = Math.min(row.indent, MAX_INDENT);
    const color = BRANCH_COLORS[row.lane % BRANCH_COLORS.length];
    const isRawTool = message.role === "tool";

    // 有落库原文用原文，否则退回前端快照（I51 §6.3 的降级路径）。
    const recordSource: RecordSource = record ? "raw" : "dto";

    const recordJson = useMemo(
      () => (viewMode === "raw" ? toRawRecordJson(record, message) : ""),
      [message, record, viewMode],
    );

    const coord =
      message.turnIndex == null
        ? t("history.detail.coord_unknown")
        : t("history.detail.coord", {
            turn: message.turnIndex,
            iteration: message.iteration,
          });

    const handleClick = useCallback(
      (event: React.MouseEvent<HTMLDivElement>) => {
        const target = event.target as HTMLElement | null;
        if (target?.closest(INTERACTIVE_SELECTOR)) return;
        onFocusNode(message.id);
      },
      [message.id, onFocusNode],
    );

    const handleCopyRecord = useCallback(() => {
      navigator.clipboard.writeText(recordJson).catch(() => {});
    }, [recordJson]);

    const handleDelete = useCallback(() => {
      onDeleteMessage(message.id);
    }, [message.id, onDeleteMessage]);

    const rowClass = [
      css.row,
      row.dimmed ? css.rowDimmed : "",
      row.onFocusPath ? "" : css.rowInteractive,
    ]
      .filter(Boolean)
      .join(" ");

    const sourceTag =
      recordError !== null ? (
        <Tooltip
          title={t("history.detail.source_raw_failed", { reason: recordError })}
        >
          <Tag color="error" style={{ margin: 0 }}>
            {t("history.detail.source_dto")}
          </Tag>
        </Tooltip>
      ) : (
        <Tag
          color={recordSource === "raw" ? "blue" : "default"}
          style={{ margin: 0 }}
        >
          {recordSource === "raw"
            ? t("history.detail.source_raw")
            : t("history.detail.source_dto")}
        </Tag>
      );

    const renderBody = () => {
      if (viewMode === "raw") {
        return (
          <div className={css.rawRow}>
            <div className={css.rawHeader}>
              <Tag color="default" style={{ margin: 0 }}>
                {message.role}
              </Tag>
              <span>{new Date(message.timestamp).toLocaleString()}</span>
              <span>{coord}</span>
              <span className={css.rawSpacer}>{message.id}</span>
              {sourceTag}
              <Tooltip title={t("history.detail.copy_record")}>
                <Button
                  size="small"
                  type="text"
                  icon={<CopyOutlined />}
                  onClick={handleCopyRecord}
                />
              </Tooltip>
              <Tooltip title={t("common.actions.delete")}>
                <Button
                  size="small"
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={handleDelete}
                />
              </Tooltip>
            </div>
            <pre className={css.rawBody}>{recordJson}</pre>
          </div>
        );
      }
      if (isRawTool) {
        return (
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
        );
      }
      return (
        <MessageBubble
          message={message}
          isStreaming={false}
          showIdentity={row.showIdentity}
          showBranchPicker={false}
          onDelete={onDeleteMessage}
        />
      );
    };

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
          <MessageMetaLine message={message} />
          {renderBody()}
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
