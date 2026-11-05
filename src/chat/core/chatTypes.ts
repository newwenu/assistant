import { type ChatError } from "./chatErrors";
import type {
  ChatDeltaPayloadDto,
  ChatMessagesChangedPayloadDto,
  ChatRegeneratePayloadDto,
  ChatStreamStartPayloadDto,
  ChatUserMessageConfirmedPayloadDto,
  ConversationDetailDto,
  ConversationDto,
  ConversationListChangeKindDto,
  ConversationListChangedPayloadDto,
  ConversationTerminatedPayloadDto,
  ConversationUnlockedPayloadDto,
  MessageChangeKindDto,
  MessageDto,
  RetryNotificationDto,
  SendChatMessageArgsDto,
  SessionStateChangedPayloadDto,
  SessionStateDto,
  SiblingInfoDto,
  TokenUsageCacheDto,
  TokenUsageCumulativeDto,
  TokenUsageDto,
  TokenUsageOccupancyDto,
  TokenUsageSummaryDto,
  TurnPhaseDto,
} from "../../types/protocol.contract";

/** 线格式 Message；`error` 是前端本地状态，不属于后端 DTO。 */
export type Message = MessageDto & { error?: ChatError | null };

export type Conversation = ConversationDto;

export type TokenUsage = TokenUsageDto;

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

export type RetryNotification = RetryNotificationDto;

export type ConversationDetail = Omit<ConversationDetailDto, "messages"> & {
  messages: Message[];
};

export type TokenUsageOccupancySummary = TokenUsageOccupancyDto;
export type TokenUsageCumulativeSummary = TokenUsageCumulativeDto;
export type TokenUsageCacheSummary = TokenUsageCacheDto;
export type TokenUsageSummary = TokenUsageSummaryDto;

export type ChatDeltaPayload = ChatDeltaPayloadDto;

export type ChatStreamStartPayload = ChatStreamStartPayloadDto;
export type ChatUserMessageConfirmedPayload =
  ChatUserMessageConfirmedPayloadDto;
export type ConversationTerminatedPayload = ConversationTerminatedPayloadDto;
export type ConversationUnlockedPayload = ConversationUnlockedPayloadDto;
export type ChatRegeneratePayload = ChatRegeneratePayloadDto;

export type SiblingInfo = SiblingInfoDto;

export type TurnPhase = TurnPhaseDto;
export type SessionState = SessionStateDto;
export type SessionStateChangedPayload = SessionStateChangedPayloadDto;

export enum ChatEvent {
  StreamStart = "chat-stream-start",
  UserMessageConfirmed = "chat-user-message-confirmed",
  Delta = "chat-delta",
  MessagesChanged = "chat-messages-changed",
  ConversationListChanged = "conversation-list-changed",
  ConversationTerminated = "conversation-terminated",
  ConversationUnlocked = "conversation-unlocked",
  SessionStateChanged = "session-state-changed",
}

export type MessageChangeKind = MessageChangeKindDto;

export type ChatMessagesChangedPayload = Omit<
  ChatMessagesChangedPayloadDto,
  "messages"
> & { messages: Message[] };

export type TruncationStrategy =
  { kind: "truncate" } | { kind: "branch"; branchPointMessageId: string };

export type ConversationListChangeKind = ConversationListChangeKindDto;

export type ConversationListChangedPayload = ConversationListChangedPayloadDto;

export type SendChatMessageArgs = SendChatMessageArgsDto;
