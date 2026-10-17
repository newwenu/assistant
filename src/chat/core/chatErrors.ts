import { translateKey } from "../../i18n";

export enum ChatErrorCode {
  ApiKeyMissing = "ApiKeyMissing",
  AuthFailed = "AuthFailed",
  RateLimited = "RateLimited",
  ModelNotFound = "ModelNotFound",
  ContextOverflow = "ContextOverflow",
  NetworkError = "NetworkError",
  Timeout = "Timeout",
  ServerError = "ServerError",
  RequestRejected = "RequestRejected",
  ParseError = "ParseError",
  Cancelled = "Cancelled",
  StreamError = "StreamError",
  InternalError = "InternalError",
}

export enum ChatErrorCategory {
  Network = "network",
  Auth = "auth",
  RateLimit = "rate_limit",
  ContextOverflow = "context_overflow",
  Timeout = "timeout",
  ProviderQuota = "provider_quota",
  ProviderUnavailable = "provider_unavailable",
  InvalidRequest = "invalid_request",
  ContentPolicy = "content_policy",
  ModelError = "model_error",
  Unknown = "unknown",
}

export interface ChatError {
  code: ChatErrorCode;
  message?: string;
  retryAfterMs: number | null;
  retryable: boolean;
  httpStatus?: number;
  provider?: string;
  providerCode?: string;
  category?: ChatErrorCategory;
}

const ERROR_ICONS: Record<ChatErrorCode, string> = {
  [ChatErrorCode.ApiKeyMissing]: "🔑",
  [ChatErrorCode.AuthFailed]: "🔐",
  [ChatErrorCode.RateLimited]: "⏳",
  [ChatErrorCode.ModelNotFound]: "🔍",
  [ChatErrorCode.ContextOverflow]: "📏",
  [ChatErrorCode.NetworkError]: "🌐",
  [ChatErrorCode.Timeout]: "⏱️",
  [ChatErrorCode.ServerError]: "🔧",
  [ChatErrorCode.RequestRejected]: "🚫",
  [ChatErrorCode.ParseError]: "📋",
  [ChatErrorCode.Cancelled]: "✋",
  [ChatErrorCode.StreamError]: "📡",
  [ChatErrorCode.InternalError]: "⚠️",
};

export function getErrorDisplay(code: ChatErrorCode): {
  icon: string;
  title: string;
} {
  return {
    icon: ERROR_ICONS[code],
    title: translateKey(`chat.error.${code}`),
  };
}

export function formatRetryAfter(ms: number | null): string {
  if (ms == null || ms <= 0) return "";
  const secs = Math.ceil(ms / 1000);
  if (secs < 90)
    return translateKey("chat.error.retry_after_seconds", { count: secs });
  const mins = Math.ceil(secs / 60);
  return translateKey("chat.error.retry_after_minutes", { count: mins });
}

export function isChatError(value: unknown): value is ChatError {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === "string" &&
    Object.values(ChatErrorCode).includes(obj.code as ChatErrorCode) &&
    (obj.message === undefined || typeof obj.message === "string") &&
    typeof obj.retryable === "boolean" &&
    (obj.retryAfterMs === undefined ||
      obj.retryAfterMs === null ||
      typeof obj.retryAfterMs === "number") &&
    (obj.httpStatus === undefined || typeof obj.httpStatus === "number") &&
    (obj.provider === undefined || typeof obj.provider === "string") &&
    (obj.providerCode === undefined || typeof obj.providerCode === "string")
  );
}

export function inferCategory(error: ChatError): ChatErrorCategory {
  const codeToCategory: Partial<Record<ChatErrorCode, ChatErrorCategory>> = {
    [ChatErrorCode.NetworkError]: ChatErrorCategory.Network,
    [ChatErrorCode.AuthFailed]: ChatErrorCategory.Auth,
    [ChatErrorCode.ApiKeyMissing]: ChatErrorCategory.Auth,
    [ChatErrorCode.RateLimited]: ChatErrorCategory.RateLimit,
    [ChatErrorCode.ContextOverflow]: ChatErrorCategory.ContextOverflow,
    [ChatErrorCode.Timeout]: ChatErrorCategory.Timeout,
    [ChatErrorCode.ServerError]: ChatErrorCategory.ProviderUnavailable,
    [ChatErrorCode.ModelNotFound]: ChatErrorCategory.ModelError,
    [ChatErrorCode.RequestRejected]: ChatErrorCategory.ContentPolicy,
    [ChatErrorCode.ParseError]: ChatErrorCategory.InvalidRequest,
    [ChatErrorCode.StreamError]: ChatErrorCategory.Network,
  };

  if (codeToCategory[error.code]) return codeToCategory[error.code]!;

  if (error.httpStatus) {
    const httpMap: Record<number, ChatErrorCategory> = {
      401: ChatErrorCategory.Auth,
      402: ChatErrorCategory.ProviderQuota,
      403: ChatErrorCategory.Auth,
      429: ChatErrorCategory.RateLimit,
      529: ChatErrorCategory.ProviderUnavailable,
    };
    if (httpMap[error.httpStatus]) return httpMap[error.httpStatus];
    if (error.httpStatus >= 500) return ChatErrorCategory.ProviderUnavailable;
  }

  const msg = (error.message ?? "").toLowerCase();
  if (msg.includes("insufficient_quota") || msg.includes("billing"))
    return ChatErrorCategory.ProviderQuota;
  if (msg.includes("context window") || msg.includes("context_length"))
    return ChatErrorCategory.ContextOverflow;
  if (msg.includes("content_policy") || msg.includes("content policy"))
    return ChatErrorCategory.ContentPolicy;

  return ChatErrorCategory.Unknown;
}

export function toChatError(value: unknown): ChatError {
  if (isChatError(value)) {
    const err: ChatError = {
      code: value.code,
      message: value.message,
      retryAfterMs: value.retryAfterMs ?? null,
      retryable: value.retryable,
      httpStatus: value.httpStatus,
      provider: value.provider,
      providerCode: value.providerCode,
    };
    err.category = inferCategory(err);
    return err;
  }
  if (typeof value === "string") {
    return {
      code: ChatErrorCode.InternalError,
      message: value,
      retryAfterMs: null,
      retryable: false,
      category: ChatErrorCategory.Unknown,
    };
  }
  return {
    code: ChatErrorCode.InternalError,
    message: translateKey("chat.error.unknown_error"),
    retryAfterMs: null,
    retryable: false,
    category: ChatErrorCategory.Unknown,
  };
}
