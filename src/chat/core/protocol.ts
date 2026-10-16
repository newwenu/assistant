/**
 * Chat protocol DTOs (frontend).
 * Mirror backend: src-tauri/src/protocol/chat_events.rs, chat_commands.rs
 * snake_case fields to match wire format.
 * These types mirror backend protocol structs and intentionally use
 * snake_case to match the Rust serde wire format.
 */

export interface MessageRow {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  tokens: number | null;
  timestamp: number;
  parent_id: string | null;
  is_pinned: boolean;
  reasoning: string | null;
  tool_calls_json: string | null;
}

export interface ConversationRow {
  id: string;
  title: string;
  title_source: string;
  provider_id: string | null;
  model_id: string;
  system_prompt: string | null;
  is_pinned: boolean;
  sort_order: number;
  generation: number;
  created_at: number;
  updated_at: number;
  is_full: boolean;
}

export interface ConversationDetailRow {
  conversation: ConversationRow;
  messages: MessageRow[];
  total_count: number;
}

export interface TokenUsageRow {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  context_window_tokens: number;
}

export interface RetryNotificationRow {
  attempt: number;
  max_attempts: number;
  delay_ms: number;
  message: string;
}

export type ProviderErrorCode =
  | "ApiKeyMissing"
  | "AuthFailed"
  | "RateLimited"
  | "ModelNotFound"
  | "ContextOverflow"
  | "NetworkError"
  | "Timeout"
  | "ServerError"
  | "RequestRejected"
  | "ParseError"
  | "Cancelled"
  | "StreamError"
  | "InternalError";

export interface ProviderErrorPayloadRow {
  code: ProviderErrorCode;
  message: string;
  retry_after_ms: number | null;
  retryable: boolean;
}

export interface ChatDeltaPayload {
  conversation_id: string;
  attempt_id: string;
  message_id: string;
  seq: number;
  delta_text?: string;
  delta_reasoning?: string;
  delta_tool_call_id?: string;
  delta_tool_call_name?: string;
  delta_tool_call_arguments?: string;
  finish_reason?: string;
  usage?: TokenUsageRow;
  error?: ProviderErrorPayloadRow;
  delta_retry?: RetryNotificationRow;
}

export interface ChatStreamStartPayload {
  conversation_id: string;
  attempt_id: string;
  message_id: string;
}

export interface ConversationTerminatedPayload {
  conversation_id: string;
  summary: string;
  is_full: boolean;
}

export interface ConversationCompactFailedPayload {
  conversation_id: string;
  error: string;
}

export interface ConversationUnlockedPayload {
  conversation_id: string;
}

export interface ChatRegeneratePayload {
  conversation_id: string;
  deleted_message_id: string;
}
