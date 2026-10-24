import { type ChatError } from "./chatErrors";

export interface Message {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parentId?: string | null;
  siblingIndex: number;
  siblingCount?: number;
  isPinned?: boolean;
  providerId?: string | null;
  modelId?: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Conversation {
  id: string;
  title: string;
  titleSource: string;
  providerId: string | null;
  modelId: string;
  systemPrompt: string | null;
  isPinned: boolean;
  sortOrder: number;
  generation: number;
  createdAt: number;
  updatedAt: number;
  isFull: boolean;
  activeBranchId: string | null;
}

export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  contextWindowTokens?: number;
}

export interface ContextUsage {
  usedTokens: number;
  maxTokens: number;
  percentage: number;
}

export interface RetryNotification {
  attempt: number;
  maxAttempts: number;
  delayMs: number;
  message: string;
  errorCode?: string;
}

export interface ConversationDetail {
  conversation: Conversation;
  messages: Message[];
  totalCount: number;
}

export interface TokenUsageSummary {
  totalTokens: number;
  contextWindowTokens: number;
  percentage: number;
}

export interface ChatDeltaPayload {
  conversationId: string;
  attemptId: string;
  messageId: string;
  seq: number;
  deltaText?: string;
  deltaReasoning?: string;
  deltaToolCallId?: string;
  deltaToolCallName?: string;
  deltaToolCallArguments?: string;
  finishReason?: string;
  usage?: TokenUsage;
  error?: ChatError;
  deltaRetry?: RetryNotification;
}

export interface ChatStreamStartPayload {
  conversationId: string;
  attemptId: string;
  messageId: string;
}

export interface ChatUserMessageConfirmedPayload {
  conversationId: string;
  userMessageId: string;
  userMessageContent: string;
}

export interface ConversationTerminatedPayload {
  conversationId: string;
  summary: string;
  isFull: boolean;
}

export interface ConversationCompactFailedPayload {
  conversationId: string;
  error: string;
}

export interface ConversationUnlockedPayload {
  conversationId: string;
}

export interface ChatRegeneratePayload {
  conversationId: string;
  deletedMessageId: string;
}

export interface SiblingInfo {
  id: string;
  siblingIndex: number;
  contentPreview: string;
}

export enum ChatEvent {
  StreamStart = "chat-stream-start",
  UserMessageConfirmed = "chat-user-message-confirmed",
  Delta = "chat-delta",
  MessagesChanged = "chat-messages-changed",
  ConversationListChanged = "conversation-list-changed",
  ConversationTerminated = "conversation-terminated",
  ConversationCompactFailed = "conversation-compact-failed",
  ConversationUnlocked = "conversation-unlocked",
}

export type MessageChangeKind =
  | "branch_created"
  | "branch_switched"
  | "branch_forked"
  | "message_deleted"
  | "messages_compacted"
  | "stream_finished";

export interface ChatMessagesChangedPayload {
  conversationId: string;
  changeKind: MessageChangeKind;
  messages: Message[];
  activeBranchId: string | null;
  _seq?: number;
}

export type TruncationStrategy =
  { kind: "truncate" } | { kind: "branch"; branchPointMessageId: string };

export type ConversationListChangeKind =
  | "renamed"
  | "deleted"
  | "pin_toggled"
  | "duplicated"
  | "model_updated"
  | "provider_updated"
  | "batch_deleted"
  | "purged"
  | "created"
  | "stream_finished";

export interface ConversationListChangedPayload {
  changeKind: ConversationListChangeKind;
  conversationId: string | null;
  affectedIds?: string[];
}
