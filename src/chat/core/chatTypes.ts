import { type ChatError } from "./chatErrors";
import type {
  AgentQuestionAnswerDto,
  AgentQuestionAnswerItemDto,
  AgentQuestionDto,
  AgentQuestionOptionDto,
  AgentQuestionRequestDto,
  AgentToolCallDto,
  AgentToolCallPayloadDto,
  ApprovalContextDto,
  ApprovalExtensionDto,
  ApprovalExtensionSeverityDto,
  ApprovalPreviewDto,
  ApprovalReasonDto,
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
  ConversationToolConfigDto,
  ConversationUnlockedPayloadDto,
  MessageChangeKindDto,
  MessageDto,
  MessageToolCallDto,
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
  ToolApprovalRequestDto,
  ToolApprovalDecisionDto,
  ToolErrorKindDto,
  TurnPhaseDto,
} from "../../types/protocol.contract";

/** 工具调用卡片状态（I38d §3.1）。历史只允许 `success` / `error` / `cancelled` / `missing`。 */
export type ToolCallStatus =
  "running" | "success" | "error" | "awaiting_user" | "cancelled" | "missing";

/**
 * 工具调用的前端视图，挂在消息上随消息一起渲染。
 * `arguments` 在 `awaitingApproval` 态直接取 `approval.arguments`（后端已解析），
 * `running` 态由 `argsText` 增量累积后解析出的最近一次可解析快照。
 */
export interface ToolCallView {
  callId: string;
  name: string;
  status: ToolCallStatus;
  arguments?: Record<string, unknown>;
  /** `argsDelta` 累积的原始文本；解析失败不影响预览 */
  argsText?: string | null;
  resultText?: string | null;
  startedAt?: number | null;
  finishedAt?: number | null;
  approval?: ToolApprovalRequestDto | null;
  question?: AgentQuestionRequestDto | null;
  errorKind?: ToolErrorKindDto | null;
  confirmationWaitMs?: number | null;
}

/** 线格式 Message；wire 的 `toolCalls` 由 `toChatMessage` 转为前端视图，`error` 为前端本地状态。 */
export type Message = Omit<MessageDto, "toolCalls"> & {
  error?: ChatError | null;
  toolCalls?: ToolCallView[];
};

export type MessageToolCall = MessageToolCallDto;

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

export type AgentToolCall = AgentToolCallDto;
export type AgentToolCallPayload = AgentToolCallPayloadDto;

export type ToolApprovalRequest = ToolApprovalRequestDto;
export type ToolApprovalDecision = ToolApprovalDecisionDto;
export type AgentQuestionRequest = AgentQuestionRequestDto;
export type AgentQuestion = AgentQuestionDto;
export type AgentQuestionOption = AgentQuestionOptionDto;
export type AgentQuestionAnswer = AgentQuestionAnswerDto;
export type AgentQuestionAnswerItem = AgentQuestionAnswerItemDto;
export type ApprovalContext = ApprovalContextDto;
export type ApprovalPreview = ApprovalPreviewDto;
export type ApprovalExtension = ApprovalExtensionDto;
export type ApprovalExtensionSeverity = ApprovalExtensionSeverityDto;
export type ApprovalReason = ApprovalReasonDto;

export enum ChatEvent {
  StreamStart = "chat-stream-start",
  UserMessageConfirmed = "chat-user-message-confirmed",
  Delta = "chat-delta",
  MessagesChanged = "chat-messages-changed",
  ConversationListChanged = "conversation-list-changed",
  ConversationTerminated = "conversation-terminated",
  ConversationUnlocked = "conversation-unlocked",
  SessionStateChanged = "session-state-changed",
  AgentToolCall = "agent-tool-call",
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

/** 随发送请求下发的会话工具配置；仅 agent 链路存在（I38d §4.8）。 */
export type ConversationToolConfig = ConversationToolConfigDto;
