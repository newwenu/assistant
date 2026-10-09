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
  message: string;
  retry_after_ms: number | null;
  retryable: boolean;
}

export const ERROR_DISPLAY: Record<
  ChatErrorCode,
  { icon: string; title: string }
> = {
  [ChatErrorCode.ApiKeyMissing]: { icon: "🔑", title: "API Key 未配置" },
  [ChatErrorCode.AuthFailed]: { icon: "🔐", title: "认证失败" },
  [ChatErrorCode.RateLimited]: { icon: "⏳", title: "请求限流" },
  [ChatErrorCode.ModelNotFound]: { icon: "🔍", title: "模型不可用" },
  [ChatErrorCode.ContextOverflow]: { icon: "📏", title: "上下文过长" },
  [ChatErrorCode.NetworkError]: { icon: "🌐", title: "网络错误" },
  [ChatErrorCode.Timeout]: { icon: "⏱️", title: "请求超时" },
  [ChatErrorCode.ServerError]: { icon: "🔧", title: "服务端错误" },
  [ChatErrorCode.RequestRejected]: { icon: "🚫", title: "请求被拒绝" },
  [ChatErrorCode.ParseError]: { icon: "📋", title: "解析错误" },
  [ChatErrorCode.Cancelled]: { icon: "✋", title: "已取消" },
  [ChatErrorCode.StreamError]: { icon: "📡", title: "流式错误" },
  [ChatErrorCode.InternalError]: { icon: "⚠️", title: "内部错误" },
};

export function formatRetryAfter(ms: number | null): string {
  if (ms == null || ms <= 0) return "";
  const secs = Math.ceil(ms / 1000);
  if (secs < 90) return `请 ${secs} 秒后重试`;
  const mins = Math.ceil(secs / 60);
  return `请约 ${mins} 分钟后重试`;
}

export function isChatError(value: unknown): value is ChatError {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === "string" &&
    Object.values(ChatErrorCode).includes(obj.code as ChatErrorCode) &&
    typeof obj.message === "string" &&
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
    message: "未知错误",
    retry_after_ms: null,
    retryable: false,
  };
}
