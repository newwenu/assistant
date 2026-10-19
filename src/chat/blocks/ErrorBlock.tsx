import React from "react";
import {
  RedoOutlined,
  SettingOutlined,
  SwapOutlined,
  CopyOutlined,
} from "@ant-design/icons";
import {
  type ChatError,
  ChatErrorCode,
  getErrorDisplay,
  formatRetryAfter,
} from "../core/chatErrors";
import {
  type RecoveryActionCode,
  getErrorPresentation,
} from "../core/chatErrorRecovery";
import { translateKey } from "../../i18n";
import css from "./ErrorBlock.module.css";

interface ErrorBlockProps {
  error: ChatError;
  onRetry?: () => void;
  onAction?: (actionCode: RecoveryActionCode) => void;
}

const ACTION_ICONS: Partial<Record<RecoveryActionCode, React.ComponentType>> = {
  retry: RedoOutlined,
  wait_and_retry: RedoOutlined,
  switch_model: SwapOutlined,
  open_model_settings: SettingOutlined,
  copy_diagnostics: CopyOutlined,
};

export const ErrorBlock: React.FC<ErrorBlockProps> = ({
  error,
  onRetry,
  onAction,
}) => {
  const display =
    getErrorDisplay(error.code) ?? getErrorDisplay(ChatErrorCode.InternalError);
  const retryHint = formatRetryAfter(error.retryAfterMs);
  const presentation = getErrorPresentation(error);

  const handleAction = (code: RecoveryActionCode) => {
    if (code === "retry" || code === "wait_and_retry") {
      onRetry?.();
    } else {
      onAction?.(code);
    }
  };

  const headerClass =
    presentation.severity === "warning"
      ? `${css.header} ${css.headerWarning}`
      : css.header;

  return (
    <div className={css.root}>
      <div className={headerClass}>
        <span className={css.icon}>{display.icon}</span>
        <span className={css.title}>{translateKey(presentation.titleKey)}</span>
      </div>
      <div className={css.suggestion}>
        {translateKey(presentation.messageKey)}
      </div>
      {error.message && <div className={css.message}>{error.message}</div>}
      {retryHint && <div className={css.retryHint}>{retryHint}</div>}
      <div className={css.actions}>
        {presentation.actions.map((action) => {
          const Icon = ACTION_ICONS[action.code];
          return (
            <button
              key={action.code}
              className={css.retryBtn}
              onClick={() => handleAction(action.code)}
              type="button"
            >
              {Icon && (
                <span
                  style={{
                    fontSize: 12,
                    marginRight: 4,
                    display: "inline-flex",
                  }}
                >
                  <Icon />
                </span>
              )}
              {translateKey(action.labelKey)}
            </button>
          );
        })}
      </div>
    </div>
  );
};
