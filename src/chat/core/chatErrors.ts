import type { CommandErrorCodeDto } from "../../types/protocol.contract";
import { translateKey } from "../../i18n";

/**
 * 错误码的唯一来源是生成契约 `CommandErrorCodeDto`（Rust 侧
 * `CommandErrorCodeDto`）。此处不再手写第二份枚举，只保留值等于键的
 * 对象，让既有 `ChatErrorCode.X` 属性访问写法继续成立。
 *
 * `SameKeys` 同时约束「漏键 / 多键」与「值写错」：后端新增码后前端
 * 未同步时 `npm run typecheck` 直接失败。
 */
type SameKeys = { [K in CommandErrorCodeDto]: K };

export const ChatErrorCode: SameKeys = {
  ApiKeyMissing: "ApiKeyMissing",
  AuthFailed: "AuthFailed",
  RateLimited: "RateLimited",
  ModelNotFound: "ModelNotFound",
  ContextOverflow: "ContextOverflow",
  NetworkError: "NetworkError",
  Timeout: "Timeout",
  ServerError: "ServerError",
  RequestRejected: "RequestRejected",
  ParseError: "ParseError",
  Cancelled: "Cancelled",
  StreamError: "StreamError",
  InternalError: "InternalError",
  SessionBusy: "SessionBusy",
  ConversationNotFound: "ConversationNotFound",
  ProviderNotFound: "ProviderNotFound",
  Validation: "Validation",
  StorageFailure: "StorageFailure",
  NoPendingInteraction: "NoPendingInteraction",
};

export type ChatErrorCode = CommandErrorCodeDto;

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
  Busy = "busy",
  Storage = "storage",
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
  // 不校验码是否在枚举内：契约生成漏跑 / 手工改生成物时仍要保留后端文案，
  // 分类由 inferCategory 回落 Unknown，message 原样透出。
  return (
    typeof obj.code === "string" &&
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

/**
 * 错误码到分类的完整映射：`satisfies Record<ChatErrorCode, ChatErrorCategory>`
 * 保证新增错误码时编译期报错，不再依赖 `Partial` + 运行时兜底。
 */
const CODE_TO_CATEGORY = {
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
  [ChatErrorCode.SessionBusy]: ChatErrorCategory.Busy,
  [ChatErrorCode.ConversationNotFound]: ChatErrorCategory.InvalidRequest,
  [ChatErrorCode.ProviderNotFound]: ChatErrorCategory.InvalidRequest,
  [ChatErrorCode.Validation]: ChatErrorCategory.InvalidRequest,
  [ChatErrorCode.StorageFailure]: ChatErrorCategory.Storage,
  [ChatErrorCode.NoPendingInteraction]: ChatErrorCategory.Cancelled,
} as const satisfies Record<ChatErrorCode, ChatErrorCategory>;

export function inferCategory(error: ChatError): ChatErrorCategory {
  // 未知码（契约漏跑 / 手工改生成物）回落 Unknown，但 message 原样保留。
  return CODE_TO_CATEGORY[error.code] ?? ChatErrorCategory.Unknown;
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
