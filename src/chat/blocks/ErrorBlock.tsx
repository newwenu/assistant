import React from "react";
import { RedoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  type ChatError,
  ChatErrorCode,
  getErrorDisplay,
  formatRetryAfter,
} from "../core/chatErrors";
import css from "./ErrorBlock.module.css";

interface ErrorBlockProps {
  error: ChatError;
  onRetry?: () => void;
}

export const ErrorBlock: React.FC<ErrorBlockProps> = ({ error, onRetry }) => {
  const display =
    getErrorDisplay(error.code) ?? getErrorDisplay(ChatErrorCode.InternalError);
  const retryHint = formatRetryAfter(error.retryAfterMs);
  const { t } = useTranslation();

  return (
    <div className={css.root}>
      <div className={css.header}>
        <span className={css.icon}>{display.icon}</span>
        <span className={css.title}>{display.title}</span>
      </div>
      {error.message && <div className={css.message}>{error.message}</div>}
      {retryHint && <div className={css.retryHint}>{retryHint}</div>}
      {error.retryable && onRetry && (
        <button className={css.retryBtn} onClick={onRetry} type="button">
          <RedoOutlined style={{ fontSize: 12, marginRight: 4 }} />
          {t("common.actions.retry")}
        </button>
      )}
    </div>
  );
};
