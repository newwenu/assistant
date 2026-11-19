import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Button, Tag, Tooltip } from "antd";
import { AimOutlined, CopyOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { MessageBubble } from "../../chat/message/MessageBubble";
import { BRANCH_COLORS, type HistoryMessageRow } from "./historyTree";
import { MessageMetaLine } from "./MessageMetaLine";
import { formatStamp } from "./formatStamp";
import { toRawRecordJson, type HistoryRecordState } from "./recordSnapshot";
import css from "./HistoryBranchView.module.css";

const MAX_INDENT = 6;

interface HistoryBranchRowProps {
  row: HistoryMessageRow;
  recordState: HistoryRecordState;
  isFocusLeaf: boolean;
  onDeleteMessage: (id: string) => void;
  onFocusNode: (nodeId: string) => void;
  onRequestRecords: () => void;
}

const INTERACTIVE_SELECTOR =
  "button, a, input, textarea, select, [role='button']";

export const HistoryBranchRow: React.FC<HistoryBranchRowProps> = React.memo(
  ({
    row,
    recordState,
    isFocusLeaf,
    onDeleteMessage,
    onFocusNode,
    onRequestRecords,
  }) => {
    const { t } = useTranslation();
    const message = row.node.message;
    const indent = Math.min(row.indent, MAX_INDENT);
    const color = BRANCH_COLORS[row.lane % BRANCH_COLORS.length];
    const isRawTool = message.role === "tool";
    const [rawOpen, setRawOpen] = useState(false);
    const panelRef = useRef<HTMLDivElement | null>(null);

    const recordJson = useMemo(
      () => (rawOpen ? toRawRecordJson(recordState.record, message) : ""),
      [message, recordState.record, rawOpen],
    );

    useEffect(() => {
      if (!rawOpen) return;
      panelRef.current?.scrollIntoView?.({ block: "nearest" });
    }, [rawOpen]);

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

    const handleToggleRaw = useCallback(() => {
      if (!rawOpen) onRequestRecords();
      setRawOpen((value) => !value);
    }, [onRequestRecords, rawOpen]);

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
      recordState.error !== null ? (
        <Tooltip
          title={t("history.detail.source_raw_failed", {
            reason: recordState.error,
          })}
        >
          <Tag color="error" style={{ margin: 0 }}>
            {t("history.detail.source_dto")}
          </Tag>
        </Tooltip>
      ) : recordState.source === "loading" ? (
        <Tag color="default" style={{ margin: 0 }}>
          {t("history.detail.source_loading")}
        </Tag>
      ) : (
        <Tag
          color={recordState.source === "raw" ? "blue" : "default"}
          style={{ margin: 0 }}
        >
          {recordState.source === "raw"
            ? t("history.detail.source_raw")
            : t("history.detail.source_dto")}
        </Tag>
      );

    const renderBody = () => {
      if (isRawTool) {
        return (
          <div className={css.rawRow}>
            <div className={css.rawHeader}>
              <Tag color="default" style={{ margin: 0 }}>
                {message.role}
              </Tag>
              <span>{formatStamp(message.timestamp, true)}</span>
              <span>{coord}</span>
              <span className={css.rawId}>{message.id}</span>
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
          showTimestamp={false}
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
          <MessageMetaLine
            message={message}
            rawOpen={rawOpen}
            onToggleRaw={handleToggleRaw}
          />
          {renderBody()}
          {rawOpen && (
            <div className={css.rawPanel} ref={panelRef}>
              <div className={css.rawHeader}>
                {sourceTag}
                <span className={css.rawId}>{message.id}</span>
                <Tooltip title={t("history.detail.copy_record")}>
                  <Button
                    size="small"
                    type="text"
                    icon={<CopyOutlined />}
                    disabled={recordState.source === "loading"}
                    onClick={handleCopyRecord}
                  />
                </Tooltip>
                <button
                  type="button"
                  className={css.metaToggle}
                  onClick={handleToggleRaw}
                >
                  {t("history.detail.toggle_raw_close")}
                </button>
              </div>
              <pre className={css.rawBody}>{recordJson}</pre>
            </div>
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
