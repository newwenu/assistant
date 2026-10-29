import React, { useState, useCallback, useRef } from "react";
import {
  CopyOutlined,
  CheckOutlined,
  DeleteOutlined,
  EditOutlined,
  RedoOutlined,
  ForkOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import css from "./MessageActions.module.css";

interface MessageActionsProps {
  role: string;
  isStreaming?: boolean;
  isBranchAtLimit?: boolean;
  onCopy?: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onRegenerate?: () => void;
  onFork?: () => void;
}

const COPY_FEEDBACK_MS = 1500;

export const MessageActions: React.FC<MessageActionsProps> = ({
  role,
  isStreaming,
  isBranchAtLimit,
  onCopy,
  onDelete,
  onEdit,
  onRegenerate,
  onFork,
}) => {
  const isUser = role === "user";
  const isAssistant = role === "assistant";
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<number | null>(null);
  const { t } = useTranslation();

  const canEdit = !isStreaming && !isBranchAtLimit;
  const canRegenerate = !isStreaming && !isBranchAtLimit;
  const canFork = !isStreaming;

  const editDisabledReason = isStreaming
    ? t("chat.branch.streamingDisableEdit")
    : t("chat.branch.atLimitDisableEdit");
  const regenDisabledReason = isStreaming
    ? t("chat.branch.streamingDisableRegenerate")
    : t("chat.branch.atLimitDisableRegenerate");
  const forkDisabledReason = t("chat.branch.streamingDisableFork");

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
          title={copied ? t("common.actions.copied") : t("common.actions.copy")}
          type="button"
        >
          {copied ? (
            <CheckOutlined style={{ fontSize: 12 }} />
          ) : (
            <CopyOutlined style={{ fontSize: 12 }} />
          )}
        </button>
      )}
      {isUser && onEdit && (
        <button
          className={canEdit ? css.btn : css.btnDisabled}
          onClick={canEdit ? onEdit : undefined}
          disabled={!canEdit}
          title={canEdit ? t("chat.message.edit") : editDisabledReason}
          type="button"
        >
          <EditOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {isAssistant && onRegenerate && (
        <button
          className={canRegenerate ? css.btn : css.btnDisabled}
          onClick={canRegenerate ? onRegenerate : undefined}
          disabled={!canRegenerate}
          title={
            canRegenerate ? t("chat.message.regenerate") : regenDisabledReason
          }
          type="button"
        >
          <RedoOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {onFork && (
        <button
          className={canFork ? css.btn : css.btnDisabled}
          onClick={canFork ? onFork : undefined}
          disabled={!canFork}
          title={canFork ? t("chat.message.fork") : forkDisabledReason}
          type="button"
        >
          <ForkOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {onDelete && (
        <button
          className={css.btn}
          onClick={onDelete}
          title={t("common.actions.delete")}
          type="button"
        >
          <DeleteOutlined style={{ fontSize: 12 }} />
        </button>
      )}
    </div>
  );
};
