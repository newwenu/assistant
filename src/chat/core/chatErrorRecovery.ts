import { ChatError, ChatErrorCategory } from "./chatErrors";

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
  category: ChatErrorCategory;
  titleKey: string;
  messageKey: string;
  severity: "warning" | "error";
  retryable: boolean;
  actions: RecoveryAction[];
  diagnostics: string;
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
  [ChatErrorCategory.ContextOverflow]: ["copy_diagnostics"],
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
  [ChatErrorCategory.Unknown]: ["retry", "switch_model", "copy_diagnostics"],
};

const ACTION_LABEL_KEYS: Record<RecoveryActionCode, string> = {
  retry: "chat.error.action.retry",
  wait_and_retry: "chat.error.action.wait_and_retry",
  switch_model: "chat.error.action.switch_model",
  compress_context: "chat.error.action.compress_context",
  start_new_chat: "chat.error.action.start_new_chat",
  open_model_settings: "chat.error.action.open_model_settings",
  copy_diagnostics: "chat.error.action.copy_diagnostics",
};

const CATEGORY_INFO: Record<
  ChatErrorCategory,
  { titleKey: string; messageKey: string; severity: "warning" | "error" }
> = {
  [ChatErrorCategory.Network]: {
    titleKey: "chat.error.category.network.title",
    messageKey: "chat.error.category.network.message",
    severity: "error",
  },
  [ChatErrorCategory.Auth]: {
    titleKey: "chat.error.category.auth.title",
    messageKey: "chat.error.category.auth.message",
    severity: "error",
  },
  [ChatErrorCategory.RateLimit]: {
    titleKey: "chat.error.category.rate_limit.title",
    messageKey: "chat.error.category.rate_limit.message",
    severity: "warning",
  },
  [ChatErrorCategory.ContextOverflow]: {
    titleKey: "chat.error.category.context_overflow.title",
    messageKey: "chat.error.category.context_overflow.message",
    severity: "error",
  },
  [ChatErrorCategory.Timeout]: {
    titleKey: "chat.error.category.timeout.title",
    messageKey: "chat.error.category.timeout.message",
    severity: "warning",
  },
  [ChatErrorCategory.ProviderQuota]: {
    titleKey: "chat.error.category.provider_quota.title",
    messageKey: "chat.error.category.provider_quota.message",
    severity: "error",
  },
  [ChatErrorCategory.ProviderUnavailable]: {
    titleKey: "chat.error.category.provider_unavailable.title",
    messageKey: "chat.error.category.provider_unavailable.message",
    severity: "warning",
  },
  [ChatErrorCategory.InvalidRequest]: {
    titleKey: "chat.error.category.invalid_request.title",
    messageKey: "chat.error.category.invalid_request.message",
    severity: "error",
  },
  [ChatErrorCategory.ContentPolicy]: {
    titleKey: "chat.error.category.content_policy.title",
    messageKey: "chat.error.category.content_policy.message",
    severity: "error",
  },
  [ChatErrorCategory.ModelError]: {
    titleKey: "chat.error.category.model_error.title",
    messageKey: "chat.error.category.model_error.message",
    severity: "error",
  },
  [ChatErrorCategory.Unknown]: {
    titleKey: "chat.error.category.unknown.title",
    messageKey: "chat.error.category.unknown.message",
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
