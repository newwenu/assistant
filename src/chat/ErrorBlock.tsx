import React from "react";
import { RedoOutlined } from "@ant-design/icons";
import {
  type ChatError,
  ChatErrorCode,
  ERROR_DISPLAY,
  formatRetryAfter,
} from "./chatErrors";
import css from "./ErrorBlock.module.css";

interface ErrorBlockProps {
  error: ChatError;
  onRetry?: () => void;
}

export const ErrorBlock: React.FC<ErrorBlockProps> = ({ error, onRetry }) => {
  const display =
    ERROR_DISPLAY[error.code] ?? ERROR_DISPLAY[ChatErrorCode.InternalError];
  const retryHint = formatRetryAfter(error.retry_after_ms);

  return (
    <div className={css.root}>
      <div className={css.header}>
        <span className={css.icon}>{display.icon}</span>
        <span className={css.title}>{display.title}</span>
      </div>
      <div className={css.message}>{error.message}</div>
      {retryHint && <div className={css.retryHint}>{retryHint}</div>}
      {error.retryable && onRetry && (
        <button className={css.retryBtn} onClick={onRetry} type="button">
          <RedoOutlined style={{ fontSize: 12, marginRight: 4 }} />
          重试
        </button>
      )}
    </div>
  );
};
