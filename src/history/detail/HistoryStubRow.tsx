import React, { useCallback } from "react";
import { BranchesOutlined, WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { extractDerivedText } from "../../chat/core/message/contentBlockUtils";
import {
  BRANCH_COLORS,
  type HistoryStubRow as HistoryStubRowModel,
} from "./historyTree";
import { formatStamp } from "./formatStamp";
import css from "./HistoryBranchView.module.css";

const MAX_INDENT = 6;
const PREVIEW_LENGTH = 60;

interface HistoryStubRowProps {
  row: HistoryStubRowModel;
  onToggleStub: (key: string) => void;
  onFocusNode: (nodeId: string) => void;
}

function previewText(content: string): string {
  const text = extractDerivedText(content).replace(/\s+/g, " ").trim();
  if (text.length === 0) return "—";
  return text.length > PREVIEW_LENGTH
    ? `${text.slice(0, PREVIEW_LENGTH)}…`
    : text;
}

export const HistoryStubRow: React.FC<HistoryStubRowProps> = React.memo(
  ({ row, onToggleStub, onFocusNode }) => {
    const { t } = useTranslation();
    const isOrphan = row.orphan;
    const count = row.siblings.length;
    const indent = Math.min(row.indent, MAX_INDENT);

    const handleToggle = useCallback(
      () => onToggleStub(row.key),
      [onToggleStub, row.key],
    );

    return (
      <div className={css.stub} style={{ paddingLeft: indent * 16 }}>
        <div className={css.stubCard}>
          <button
            type="button"
            className={css.stubHeader}
            onClick={handleToggle}
          >
            {isOrphan ? (
              <WarningOutlined className={css.stubOrphan} />
            ) : (
              <BranchesOutlined />
            )}
            <span
              className={[css.stubHeaderTitle, isOrphan ? css.stubOrphan : ""]
                .filter(Boolean)
                .join(" ")}
            >
              {isOrphan
                ? t("history.tree.orphan_group", { count })
                : t("history.tree.other_branches", { count })}
            </span>
            <span>
              {row.expanded
                ? t("history.tree.collapse")
                : t("history.tree.expand")}
            </span>
          </button>
          <div className={css.stubPreviewList}>
            {row.siblings.map((sibling) => {
              const message = sibling.message;
              const color =
                BRANCH_COLORS[
                  (message.siblingCount ?? 1) > 1
                    ? message.siblingIndex % BRANCH_COLORS.length
                    : row.lane % BRANCH_COLORS.length
                ];
              return (
                <button
                  key={message.id}
                  type="button"
                  className={css.stubPreview}
                  onClick={() => onFocusNode(message.id)}
                >
                  <span
                    className={css.stubPreviewRole}
                    style={{ borderColor: color, color }}
                  >
                    {message.role}
                  </span>
                  <span className={css.stubPreviewText}>
                    {previewText(message.content)}
                  </span>
                  <span className={css.stubPreviewMeta}>
                    {formatStamp(message.timestamp)}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    );
  },
);
