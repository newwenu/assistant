import { ChatError, ChatErrorCode, ChatErrorCategory } from "./chatErrors";
import { translateKey } from "../../i18n";
import {
  KeyOutlined,
  LockOutlined,
  FieldTimeOutlined,
  SearchOutlined,
  ColumnWidthOutlined,
  GlobalOutlined,
  ClockCircleOutlined,
  ToolOutlined,
  StopOutlined,
  FileTextOutlined,
  PauseCircleOutlined,
  ApiOutlined,
  WarningOutlined,
} from "@ant-design/icons";

const ERROR_ICONS: Record<ChatErrorCode, React.ComponentType> = {
  [ChatErrorCode.ApiKeyMissing]: KeyOutlined,
  [ChatErrorCode.AuthFailed]: LockOutlined,
  [ChatErrorCode.RateLimited]: FieldTimeOutlined,
  [ChatErrorCode.ModelNotFound]: SearchOutlined,
  [ChatErrorCode.ContextOverflow]: ColumnWidthOutlined,
  [ChatErrorCode.NetworkError]: GlobalOutlined,
  [ChatErrorCode.Timeout]: ClockCircleOutlined,
  [ChatErrorCode.ServerError]: ToolOutlined,
  [ChatErrorCode.RequestRejected]: StopOutlined,
  [ChatErrorCode.ParseError]: FileTextOutlined,
  [ChatErrorCode.Cancelled]: PauseCircleOutlined,
  [ChatErrorCode.StreamError]: ApiOutlined,
  [ChatErrorCode.InternalError]: WarningOutlined,
};

export type RecoveryActionCode =
  | "retry"
  | "wait_and_retry"
  | "switch_model"
  | "compress_context"
  | "start_new_chat"
  | "open_model_settings"
  | "copy_diagnostics";

export interface RecoveryAction {
  code: RecoveryActionCode;
  labelKey: string;
}

export interface ChatErrorPresentation {
  icon: React.ComponentType;
  category: ChatErrorCategory;
  titleKey: string;
  messageKey: string;
  severity: "warning" | "error";
  retryable: boolean;
  actions: RecoveryAction[];
  diagnostics: string;
}

export function formatRetryAfter(ms: number | null): string {
  if (ms == null || ms <= 0) return "";
  const secs = Math.ceil(ms / 1000);
  if (secs < 90)
    return translateKey("chat.error.retry_after_seconds", { count: secs });
  const mins = Math.ceil(secs / 60);
  return translateKey("chat.error.retry_after_minutes", { count: mins });
}

export interface RecoveryContext {
  conversationId: string;
  failedMessageId: string;
  error: ChatError;
  switchModel: (conversationId: string) => void;
  compressContext: (conversationId: string) => Promise<void>;
  openSettings: () => void;
  startNewChat: () => void;
  retryFromError: (messageId: string) => Promise<void>;
}

const DEFAULT_ACTIONS: Record<ChatErrorCategory, RecoveryActionCode[]> = {
  [ChatErrorCategory.Network]: ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.Auth]: ["open_model_settings", "copy_diagnostics"],
  [ChatErrorCategory.RateLimit]: [
    "wait_and_retry",
    "switch_model",
    "copy_diagnostics",
  ],
  [ChatErrorCategory.ContextOverflow]: [
    "compress_context",
    "start_new_chat",
    "copy_diagnostics",
  ],
  [ChatErrorCategory.Timeout]: ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.ProviderQuota]: [
    "open_model_settings",
    "switch_model",
    "copy_diagnostics",
  ],
  [ChatErrorCategory.ProviderUnavailable]: [
    "wait_and_retry",
    "switch_model",
    "copy_diagnostics",
  ],
  [ChatErrorCategory.InvalidRequest]: ["copy_diagnostics"],
  [ChatErrorCategory.ContentPolicy]: ["copy_diagnostics"],
  [ChatErrorCategory.ModelError]: ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.Cancelled]: ["copy_diagnostics"],
  [ChatErrorCategory.Unknown]: ["retry", "switch_model", "copy_diagnostics"],
};

const ACTION_LABEL_KEYS: Record<RecoveryActionCode, string> = {
  retry: "common.actions.retry",
  wait_and_retry: "chat.error.action_wait_and_retry",
  switch_model: "chat.error.action_switch_model",
  compress_context: "chat.error.action_compress_context",
  start_new_chat: "chat.error.action_start_new_chat",
  open_model_settings: "common.actions.settings",
  copy_diagnostics: "chat.error.action_copy_diagnostics",
};

const CATEGORY_INFO: Record<
  ChatErrorCategory,
  { titleKey: string; messageKey: string; severity: "warning" | "error" }
> = {
  [ChatErrorCategory.Network]: {
    titleKey: "chat.error.NetworkError",
    messageKey: "chat.error.category_network_message",
    severity: "error",
  },
  [ChatErrorCategory.Auth]: {
    titleKey: "chat.error.AuthFailed",
    messageKey: "chat.error.category_auth_message",
    severity: "error",
  },
  [ChatErrorCategory.RateLimit]: {
    titleKey: "chat.error.RateLimited",
    messageKey: "chat.error.category_rate_limit_message",
    severity: "warning",
  },
  [ChatErrorCategory.ContextOverflow]: {
    titleKey: "chat.error.ContextOverflow",
    messageKey: "chat.error.category_context_overflow_message",
    severity: "error",
  },
  [ChatErrorCategory.Timeout]: {
    titleKey: "chat.error.Timeout",
    messageKey: "chat.error.category_timeout_message",
    severity: "warning",
  },
  [ChatErrorCategory.ProviderQuota]: {
    titleKey: "chat.error.RateLimited",
    messageKey: "chat.error.category_provider_quota_message",
    severity: "error",
  },
  [ChatErrorCategory.ProviderUnavailable]: {
    titleKey: "chat.error.ServerError",
    messageKey: "chat.error.category_provider_unavailable_message",
    severity: "warning",
  },
  [ChatErrorCategory.InvalidRequest]: {
    titleKey: "chat.error.ParseError",
    messageKey: "chat.error.category_invalid_request_message",
    severity: "error",
  },
  [ChatErrorCategory.ContentPolicy]: {
    titleKey: "chat.error.RequestRejected",
    messageKey: "chat.error.category_content_policy_message",
    severity: "error",
  },
  [ChatErrorCategory.ModelError]: {
    titleKey: "chat.error.ModelNotFound",
    messageKey: "chat.error.category_model_error_message",
    severity: "error",
  },
  [ChatErrorCategory.Cancelled]: {
    titleKey: "chat.error.Cancelled",
    messageKey: "chat.error.category_cancelled_message",
    severity: "warning",
  },
  [ChatErrorCategory.Unknown]: {
    titleKey: "chat.error.InternalError",
    messageKey: "chat.error.category_unknown_message",
    severity: "error",
  },
};

export function getErrorPresentation(error: ChatError): ChatErrorPresentation {
  const category = error.category ?? ChatErrorCategory.Unknown;
  const info = CATEGORY_INFO[category];
  const actionCodes = DEFAULT_ACTIONS[category];
  const actions: RecoveryAction[] = actionCodes.map((code) => ({
    code,
    labelKey: ACTION_LABEL_KEYS[code],
  }));

  const diagnostics = [
    `Code: ${error.code}`,
    error.httpStatus ? `HTTP ${error.httpStatus}` : null,
    error.provider ? `Provider: ${error.provider}` : null,
    error.providerCode ? `ProviderCode: ${error.providerCode}` : null,
    error.message ? `Message: ${error.message}` : null,
  ]
    .filter(Boolean)
    .join(" | ");

  return {
    icon: ERROR_ICONS[error.code] ?? ERROR_ICONS[ChatErrorCode.InternalError],
    category,
    titleKey: info.titleKey,
    messageKey: info.messageKey,
    severity: info.severity,
    retryable: error.retryable,
    actions,
    diagnostics,
  };
}

export function getRecoveryActions(
  category: ChatErrorCategory,
): RecoveryAction[] {
  return DEFAULT_ACTIONS[category].map((code) => ({
    code,
    labelKey: ACTION_LABEL_KEYS[code],
  }));
}

export async function executeRecoveryAction(
  action: RecoveryActionCode,
  context: RecoveryContext,
): Promise<void> {
  switch (action) {
    case "retry":
      await context.retryFromError(context.failedMessageId);
      break;
    case "wait_and_retry":
      await context.retryFromError(context.failedMessageId);
      break;
    case "switch_model":
      context.switchModel(context.conversationId);
      break;
    case "compress_context":
      await context.compressContext(context.conversationId);
      break;
    case "start_new_chat":
      context.startNewChat();
      break;
    case "open_model_settings":
      context.openSettings();
      break;
    case "copy_diagnostics": {
      const presentation = getErrorPresentation(context.error);
      await navigator.clipboard.writeText(presentation.diagnostics);
      break;
    }
  }
}
