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
      {isUser && onEdit && !isStreaming && (
        <button
          className={isBranchAtLimit ? css.btnDisabled : css.btn}
          onClick={isBranchAtLimit ? undefined : onEdit}
          disabled={isBranchAtLimit}
          title={
            isBranchAtLimit
              ? t("chat.branch.atLimitDisableEdit")
              : t("chat.message.edit")
          }
          type="button"
        >
          <EditOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {isAssistant && onRegenerate && !isStreaming && (
        <button
          className={isBranchAtLimit ? css.btnDisabled : css.btn}
          onClick={isBranchAtLimit ? undefined : onRegenerate}
          disabled={isBranchAtLimit}
          title={
            isBranchAtLimit
              ? t("chat.branch.atLimitDisableRegenerate")
              : t("chat.message.regenerate")
          }
          type="button"
        >
          <RedoOutlined style={{ fontSize: 12 }} />
        </button>
      )}
      {onFork && !isStreaming && (
        <button
          className={css.btn}
          onClick={onFork}
          title={t("chat.message.fork")}
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
