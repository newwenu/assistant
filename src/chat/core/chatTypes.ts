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
  reasoningTokens?: number;
  contextWindowTokens?: number;
}

export interface ContextUsage {
  /** 占用 O：最新一次 chat 请求的输入 P；`hasUsage` 为 false 时不参与展示与百分比计算 */
  usedTokens: number;
  /** 该分支上是否存在可用记录；false 时占用显示「—」 */
  hasUsage: boolean;
  /** 窗口 W；未知时必须为 undefined，不得填缺省值 */
  maxTokens?: number;
  /** 与绝对数互斥：`maxTokens` 为 undefined 时同为 undefined */
  percentage?: number;
  /** 最近一次请求的输出 C，仅 Tooltip 使用 */
  completionTokens?: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  /** 累计 A，独立于占用，不得复用 usedTokens；零记录时为 undefined */
  cumulativeTokens?: number;
  /** 含估算值，UI 加 `~` 前缀 */
  estimated?: boolean;
  /** 估算来源；`estimated` 为 true 时必有值 */
  estimatedSource?: "record" | "projection";
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

export interface TokenUsageOccupancySummary {
  usedTokens?: number;
  completionTokens?: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  contextWindowTokens?: number;
  percentage?: number;
  estimated?: boolean;
  /** 估算来源：记录表估算 / 分支零记录时的出站投影 */
  estimatedSource?: "record" | "projection";
}

export interface TokenUsageCumulativeSummary {
  totalTokens: number;
  promptTokens: number;
  completionTokens: number;
}

export interface TokenUsageCacheSummary {
  hitTokens: number;
  denominatorTokens: number;
  hitRate?: number;
}

export interface TokenUsageSummary {
  occupancy: TokenUsageOccupancySummary;
  /** 该会话零记录时缺省，UI 显示「—」 */
  cumulative?: TokenUsageCumulativeSummary;
  cache?: TokenUsageCacheSummary;
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

export interface SendChatMessageArgs {
  content: string;
  modeId?: string | null;
  branchPointMessageId?: string | null;
  providerId: string;
  modelId: string;
  reasoningLevel?: string | null;
}
