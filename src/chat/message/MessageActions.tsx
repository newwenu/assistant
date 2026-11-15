import React, { useState, useCallback, useRef } from "react";
import {
  CopyOutlined,
  CheckOutlined,
  DeleteOutlined,
  EditOutlined,
  RedoOutlined,
  ForkOutlined,
} from "@ant-design/icons";
import { App } from "antd";
import { useTranslation } from "react-i18next";
import css from "./MessageActions.module.css";

/** 写入剪贴板并返回是否成功；失败不抛出，由调用方决定提示文案。 */
export async function copyTextToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

interface MessageActionsProps {
  role: string;
  isStreaming?: boolean;
  isBranchAtLimit?: boolean;
  /** 返回 `false` 表示复制失败：此时不显示「已复制」并提示失败（I46 §4.2）。 */
  onCopy?: () => boolean | Promise<boolean>;
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
  const { message } = App.useApp();

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

  const handleCopy = useCallback(async () => {
    if (!onCopy) return;
    let succeeded = false;
    try {
      succeeded = (await onCopy()) !== false;
    } catch {
      succeeded = false;
    }
    if (!succeeded) {
      void message.error(t("common.actions.copy_failed"));
      return;
    }
    setCopied(true);
    if (timerRef.current !== null) clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(
      () => setCopied(false),
      COPY_FEEDBACK_MS,
    );
  }, [onCopy, message, t]);

  return (
    <div className={css.root}>
      {onCopy && (
        <button
          className={copied ? css.btnCopied : css.btn}
          onClick={() => void handleCopy()}
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
