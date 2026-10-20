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
  Cancelled = "cancelled",
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
  details?: Record<string, unknown>;
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
    (obj.providerCode === undefined || typeof obj.providerCode === "string") &&
    (obj.details === undefined ||
      (typeof obj.details === "object" && obj.details !== null))
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
    [ChatErrorCode.Cancelled]: ChatErrorCategory.Cancelled,
    [ChatErrorCode.InternalError]: ChatErrorCategory.Unknown,
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

export function toChatError(
  value: unknown,
  extra?: Record<string, unknown>,
): ChatError {
  if (isChatError(value)) {
    const mergedDetails: Record<string, unknown> = {};
    if (value.details) Object.assign(mergedDetails, value.details);
    if (extra) Object.assign(mergedDetails, extra);
    const err: ChatError = {
      code: value.code,
      message: value.message,
      retryAfterMs: value.retryAfterMs ?? null,
      retryable: value.retryable,
      httpStatus: value.httpStatus,
      provider: value.provider,
      providerCode: value.providerCode,
      details:
        Object.keys(mergedDetails).length > 0 ? mergedDetails : undefined,
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
      details: extra,
    };
  }
  return {
    code: ChatErrorCode.InternalError,
    message: translateKey("chat.error.unknown_error"),
    retryAfterMs: null,
    retryable: false,
    category: ChatErrorCategory.Unknown,
    details: extra,
  };
}
