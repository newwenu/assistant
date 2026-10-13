import { translateKey } from "../i18n";

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

export interface ChatError {
  code: ChatErrorCode;
  message?: string;
  retry_after_ms: number | null;
  retryable: boolean;
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
    typeof obj.retryable === "boolean"
  );
}

export function toChatError(value: unknown): ChatError {
  if (isChatError(value)) return value;
  if (typeof value === "string") {
    return {
      code: ChatErrorCode.InternalError,
      message: value,
      retry_after_ms: null,
      retryable: false,
    };
  }
  return {
    code: ChatErrorCode.InternalError,
    message: translateKey("chat.error.unknown_error"),
    retry_after_ms: null,
    retryable: false,
  };
}
