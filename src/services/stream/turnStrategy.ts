import type {
  AgentToolCall,
  ChatDeltaPayload,
  ContextUsage,
  Message,
  RetryNotification,
  TokenUsage,
} from "../../chat/core/chatTypes";
import type { ChatError } from "../../chat/core/chatErrors";
import type { StreamTextChunk } from "./transport";

/** 链路种类：只由会话 `modeId` 解析得到，是策略选择的唯一判据（I38d §4.8 / §3.7）。 */
export type TurnKind = "chat" | "agent";

/**
 * 视图状态端口：只管「当前正在流的那一条消息」。
 *
 * 刻意不出现 `iteration` / 段表 / 工具归属——这些是 agent 私有语义，
 * 由 `agentTurn` 策略与 `agentSegments` 承担（I38d §3.7）。
 */
export interface StreamStatePort {
  beginSend: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
  setActiveAttemptId: (attemptId: string, realMessageId: string) => void;
  clearActiveAttemptId: () => void;
  resumeStream: (attemptId: string, messageId: string | null) => void;
  confirmUserMessage: (
    userMessageId: string,
    userMessageContent: string,
  ) => void;
  /** 单缓冲追加：chat 语义下即整轮唯一气泡。 */
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  /** 推理增量追加到当前流式消息；agent 由策略按迭代归属后再调用。 */
  appendReasoningDelta: (delta: string) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  setErrorOnMessage: (messageId: string, error: ChatError) => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
}

/**
 * 链路策略：两条链路唯一允许分叉的地方。
 *
 * 判据是「把 agent 链路整个删掉，这段逻辑还要不要」——要则留在编排骨架，
 * 不要则下沉到这里（I38d §3.7）。
 */
export interface TurnStrategy {
  readonly kind: TurnKind;
  /** 新一轮开始：把首个占位消息绑定到迭代 1（chat 无操作）。 */
  onBeginSend: (assistantMsgId: string) => void;
  /** 文本增量：chat 单缓冲累加；agent 按 `iteration` 分段（§3.1.1）。 */
  onTextDelta: (chunk: StreamTextChunk) => void;
  onReasoningDelta: (chunk: ChatDeltaPayload) => void;
  /** 模型单次调用收尾：chat 即 turn 结束；agent 只提交本段（§3.1.1）。 */
  onFinishReason: (payload: ChatDeltaPayload) => void;
  /** 增量流内错误：把错误挂到消息上并结束流式态。 */
  onDeltaError: (error: ChatError) => void;
  /** `agent-tool-call` 事件：chat 无工具，直接丢弃。 */
  onToolCall: (call: AgentToolCall) => void;
  /** turn 正常结束（`session-state-changed idle`）。 */
  onTurnIdle: () => void;
  /** turn 异常结束（`session-state-changed error`）。 */
  onTurnError: () => void;
  /** 用户显式取消。 */
  onCancel: () => void;
  /** 切回运行中的会话：`messageId` 为 `null` 表示真实行 id 尚不可知。 */
  onResume: (messageId: string | null) => void;
  /** 真实行 id 就绪：`previousId` 是占位 id。 */
  onMessageIdResolved: (previousId: string, realMessageId: string) => void;
}
