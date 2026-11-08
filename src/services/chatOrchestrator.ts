import { chatService } from "./chatService";
import { providerService } from "./providerService";
import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
  AgentToolCall,
  ChatDeltaPayload,
  RetryNotification,
  SendChatMessageArgs,
  SessionStateChangedPayload,
  SessionState,
  TurnPhase,
} from "../chat/core/chatTypes";
import type { ConversationRoute } from "../chat/core/conversationLink";
import { extractPlainText } from "../chat/core/contentBlockUtils";
import { contextUsageFromTokenUsage } from "../chat/core/chatUtils";
import { toReasoningLevel, type ReasoningChoice } from "../chat/core/reasoning";
import type { ChatError } from "../chat/core/chatErrors";
import { ChatErrorCode, toChatError } from "../chat/core/chatErrors";
import { CHAT_AUTO_TITLE_LENGTH } from "../config/constants";
import { createLogger } from "./logger";

const log = createLogger("orchestrator");

export type ChatPhase =
  "idle" | "sending" | "streaming" | "tool_executing" | "error";

export interface ChatStatePort {
  beginSend: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
  setActiveAttemptId: (attemptId: string, realMessageId: string) => void;
  clearActiveAttemptId: () => void;
  resumeStream: (attemptId: string, messageId: string | null) => void;
  confirmUserMessage: (
    userMessageId: string,
    userMessageContent: string,
  ) => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  appendReasoningDelta: (messageId: string, delta: string) => void;
  applyToolCall: (messageId: string, call: AgentToolCall) => void;
  settleToolCalls: (messageId: string) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  setErrorOnMessage: (messageId: string, error: ChatError) => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
}

export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;
  getStreamingMessageId: () => string | null;
  getActiveAttemptId: () => string | null;
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getConversationRoute: (conversationId: string) => ConversationRoute;
  getReasoningChoice: () => ReasoningChoice;
  getSelectedProviderId: () => string | null;
  getSelectedModelId: () => string;
  getPendingBranchPoint: () => string | null;
  clearPendingBranchPoint: () => void;
}

export interface ChatOrchestratorDeps {
  state: ChatStatePort;
  env: ChatEnvPort;
  loadConversations: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ) => Promise<void>;
}

export interface ChatOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  onTextDelta: (chunk: ChatTextChunk) => void;
  onStreamStart: (
    conversationId: string,
    attemptId: string,
    messageId: string,
  ) => void;
  onUserMessageConfirmed: (
    userMessageId: string,
    userMessageContent: string,
  ) => void;
  onAgentToolCall: (call: AgentToolCall) => void;
  onSessionState: (payload: SessionStateChangedPayload) => void;
  /**
   * 切换会话：**不取消**后台 turn，只把本地流式视图换成目标会话的 phase（C5.2 / A4）。
   * 目标会话仍在运行时恢复流式视图，`messageId` 由后续 delta 补全。
   */
  onConversationSwitch: (conversationId: string) => void;
  getSendInProgress: () => boolean;
  getPhase: () => ChatPhase;
  getActiveConversationId: () => string | null;
}

/** 文本增量：携带会话 / 轮次标识，供编排层按会话路由（C5.2）。 */
export interface ChatTextChunk {
  conversationId: string;
  attemptId: string;
  messageId: string;
  delta: string;
}

function fallbackTitleFromUserMessage(
  userMessage: string,
  maxLength = CHAT_AUTO_TITLE_LENGTH,
): string {
  const plainText = extractPlainText(userMessage);
  const firstLine = plainText.split("\n").find((l) => l.trim()) || "";
  if (firstLine.length <= maxLength) return firstLine.trim();
  const puncts = ["。", "？", "！", ".", "?", "!", "；", ";", "，", ","];
  for (const p of puncts) {
    const idx = firstLine.indexOf(p);
    if (idx > 0 && idx < maxLength) {
      return firstLine.slice(0, idx + 1).trim();
    }
  }
  return firstLine.slice(0, maxLength).trim() + "…";
}

/** 后端 `TurnPhaseDto` → UI `ChatPhase`（I38d §2.3：phase 由后端驱动，前端不再自行推导）。 */
function phaseFromTurnPhase(turnPhase: TurnPhase): ChatPhase {
  switch (turnPhase) {
    case "starting":
      return "sending";
    case "toolCalling":
    case "awaitingApproval":
      return "tool_executing";
    case "compacting":
    case "thinking":
    case "streaming":
    case "finalizing":
      return "streaming";
  }
}

/** 后端 `SessionStateDto` → UI `ChatPhase`；`error` 是终态（等下一次 `session-state-changed` 复位）。 */
function stateToPhase(state: SessionState): ChatPhase {
  if (state === "idle") return "idle";
  if ("processing" in state) return phaseFromTurnPhase(state.processing.phase);
  return "error";
}

export function createChatOrchestrator(
  deps: ChatOrchestratorDeps,
): ChatOrchestrator {
  let sendInProgress = false;
  let phase: ChatPhase = "idle";
  /**
   * 各会话的 phase 与当前轮次 id，唯一来源是 `session-state-changed`（I38d §3.5 第 3 步）。
   * 切会话不取消后台 turn，因此必须按会话记录，切回时才能还原（C5.2 / C5.3）。
   */
  const phaseByConversation = new Map<string, ChatPhase>();
  const turnIdByConversation = new Map<string, string>();

  function setPhase(next: ChatPhase) {
    phase = next;
  }

  /** 运行中 = 除 `idle` / `error` 之外的 phase。 */
  function isRunning(next: ChatPhase): boolean {
    return next !== "idle" && next !== "error";
  }

  /**
   * 取消按会话自身链路分派命令：链路只由会话 `modeId` 决定，
   * 与发送使用同一判据，避免中途切模式造成命令与链路不一致。
   */
  async function cancelTurn(conversationId: string) {
    const route = deps.env.getConversationRoute(conversationId);
    if (route.link === "agent") {
      await chatService.cancelAgent(conversationId);
    } else {
      await chatService.cancelChat(conversationId);
    }
  }

  function settleActiveToolCalls() {
    const messageId = deps.env.getStreamingMessageId();
    if (messageId) deps.state.settleToolCalls(messageId);
  }

  async function send(content: string) {
    if (sendInProgress) return;
    sendInProgress = true;

    try {
      if (deps.env.getIsStreaming()) {
        const prevConvId = deps.env.getActiveConversationId();
        if (prevConvId) {
          await cancelTurn(prevConvId).catch((e) =>
            log.warn("cancelTurn failed in send", e),
          );
        }
        settleActiveToolCalls();
        deps.state.clearActiveAttemptId();
        deps.state.cancelStreaming();
      }

      let convId = deps.env.getActiveConversationId();
      if (!convId) {
        const provList = await providerService.listProviders();
        if (provList.length === 0) {
          const errConvId = `err-${Date.now()}`;
          const userMsg: Message = {
            id: `temp-user-${Date.now()}`,
            conversationId: errConvId,
            role: "user",
            content,
            tokens: null,
            timestamp: Date.now(),
            parentId: null,
            isPinned: false,
            reasoning: null,
            siblingIndex: 0,
            siblingCount: null,
            providerId: null,
            modelId: null,
          };
          const assistantMsgId = `temp-assistant-${Date.now()}`;
          const assistantMsg: Message = {
            id: assistantMsgId,
            conversationId: errConvId,
            role: "assistant",
            content: "",
            tokens: null,
            timestamp: Date.now() + 1,
            parentId: null,
            isPinned: false,
            reasoning: null,
            siblingIndex: 0,
            siblingCount: null,
            providerId: null,
            modelId: null,
          };
          deps.state.beginSend([userMsg, assistantMsg], assistantMsgId);
          deps.state.errorStream({
            code: ChatErrorCode.ApiKeyMissing,
            retryAfterMs: null,
            retryable: false,
          });
          return;
        }
        const p = provList[0];
        const newId = await chatService.createConversation(
          p.id,
          p.defaultModel || "",
          "builtin-default",
        );
        await deps.loadConversations();
        await deps.switchConversation(newId);
        convId = newId;
      }

      if (!convId) {
        deps.state.errorStream({
          code: ChatErrorCode.InternalError,
          message: "No conversation available",
          retryAfterMs: null,
          retryable: false,
        });
        return;
      }

      const currentMessages = deps.env.getMessages();
      const pendingBranchPoint = deps.env.getPendingBranchPoint();
      if (pendingBranchPoint) {
        deps.env.clearPendingBranchPoint();
      }

      const branchPointMsg = pendingBranchPoint
        ? currentMessages.find((m) => m.id === pendingBranchPoint)
        : undefined;

      const userParentId = branchPointMsg
        ? branchPointMsg.parentId
        : (currentMessages[currentMessages.length - 1]?.id ?? null);

      let optimisticMessages: Message[];

      if (branchPointMsg) {
        const branchPointIndex = currentMessages.findIndex(
          (m) => m.id === pendingBranchPoint,
        );
        const baseMessages =
          branchPointIndex >= 0
            ? currentMessages.slice(0, branchPointIndex)
            : currentMessages;
        optimisticMessages = [...baseMessages];
      } else {
        optimisticMessages = [...currentMessages];
      }

      if (branchPointMsg && branchPointMsg.role === "assistant") {
        const assistantMsgId = `temp-assistant-${Date.now()}`;
        const assistantMsg: Message = {
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          tokens: null,
          timestamp: Date.now(),
          parentId: userParentId,
          isPinned: false,
          reasoning: null,
          siblingIndex: 0,
          siblingCount: null,
          providerId: null,
          modelId: null,
        };
        optimisticMessages.push(assistantMsg);
        deps.state.beginSend(optimisticMessages, assistantMsgId);
      } else {
        const userMsg: Message = {
          id: `temp-user-${Date.now()}`,
          conversationId: convId,
          role: "user",
          content,
          tokens: null,
          timestamp: Date.now(),
          parentId: userParentId,
          isPinned: false,
          reasoning: null,
          siblingIndex: 0,
          siblingCount: null,
          providerId: null,
          modelId: null,
        };
        const assistantMsgId = `temp-assistant-${Date.now()}`;
        const assistantMsg: Message = {
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          tokens: null,
          timestamp: Date.now() + 1,
          parentId: userMsg.id,
          isPinned: false,
          reasoning: null,
          siblingIndex: 0,
          siblingCount: null,
          providerId: null,
          modelId: null,
        };
        optimisticMessages.push(userMsg, assistantMsg);
        deps.state.beginSend(optimisticMessages, assistantMsgId);
      }

      const convs = deps.env.getConversations();
      const conv = convs.find((c) => c.id === convId);
      if (conv && conv.titleSource === "placeholder") {
        const fbTitle = fallbackTitleFromUserMessage(content);
        void deps.renameConversation(convId, fbTitle, "fallback");
      }

      const route = deps.env.getConversationRoute(convId);
      try {
        const selectedProviderId = deps.env.getSelectedProviderId();
        const selectedModelId = deps.env.getSelectedModelId();
        if (!selectedProviderId) {
          deps.state.errorStream({
            code: ChatErrorCode.ApiKeyMissing,
            retryAfterMs: null,
            retryable: false,
          });
          return;
        }
        const reasoningLevel = toReasoningLevel(deps.env.getReasoningChoice());
        const args: SendChatMessageArgs = {
          content,
          modeId: route.modeId,
          branchPointMessageId: pendingBranchPoint,
          providerId: selectedProviderId,
          modelId: selectedModelId,
          reasoningLevel,
          toolConfig: route.toolConfig,
        };
        if (route.link === "agent") {
          await chatService.sendAgentMessage(convId, args);
        } else {
          await chatService.sendChatMessage(convId, args);
        }
      } catch (e) {
        log.error("sendMessage failed", e);
        deps.state.errorStream(
          toChatError(e, {
            command:
              route.link === "agent"
                ? "send_agent_message"
                : "send_chat_message",
            conversationId: convId,
          }),
        );
      }
    } finally {
      sendInProgress = false;
    }
  }

  async function cancel() {
    const convId = deps.env.getActiveConversationId();
    if (!convId) return;
    const streamingMsgId = deps.env.getStreamingMessageId();
    try {
      await cancelTurn(convId);
    } catch (e) {
      log.error("cancel failed", e);
    }
    if (streamingMsgId) deps.state.settleToolCalls(streamingMsgId);
    deps.state.cancelStreaming();
    turnIdByConversation.delete(convId);
    phaseByConversation.delete(convId);
    if (streamingMsgId) {
      deps.state.setErrorOnMessage(
        streamingMsgId,
        toChatError({
          code: ChatErrorCode.Cancelled,
          retryAfterMs: null,
          retryable: false,
        }),
      );
    }
    setPhase("idle");
  }

  function handleDeltaEvent(payload: ChatDeltaPayload) {
    if (payload.conversationId !== deps.env.getActiveConversationId()) return;

    const activeAttemptId = deps.env.getActiveAttemptId();
    if (activeAttemptId === null || payload.attemptId !== activeAttemptId) {
      return;
    }

    /** 切回运行中的会话时 `messageId` 未知，由首个非文本增量补全。 */
    if (deps.env.getStreamingMessageId() === null) {
      deps.state.setActiveAttemptId(payload.attemptId, payload.messageId);
    }

    if (payload.error) {
      settleActiveToolCalls();
      deps.state.errorStream(toChatError(payload.error));
      return;
    }

    if (payload.deltaRetry) {
      deps.state.setRetryStatus(payload.deltaRetry);
      return;
    }

    if (payload.deltaReasoning) {
      const messageId = deps.env.getStreamingMessageId();
      if (messageId) {
        deps.state.setReasoningStreaming(true);
        deps.state.appendReasoningDelta(messageId, payload.deltaReasoning);
      }
    }

    if (payload.usage) {
      deps.state.updateContextUsage(contextUsageFromTokenUsage(payload.usage));
    }

    if (payload.finishReason) {
      deps.state.setRetryStatus(null);
      settleActiveToolCalls();
      deps.state.finishStream(payload.usage ?? undefined);
    }
  }

  /**
   * 文本增量按会话路由：后台会话的增量不落到当前会话的流式缓冲（C5.2）。
   * phase 由 `session-state-changed` 驱动，这里不再自行推导（C5.3）。
   */
  function onTextDelta(chunk: ChatTextChunk) {
    if (chunk.conversationId !== deps.env.getActiveConversationId()) return;
    const activeAttemptId = deps.env.getActiveAttemptId();
    if (activeAttemptId === null || chunk.attemptId !== activeAttemptId) {
      return;
    }
    if (deps.env.getStreamingMessageId() === null) {
      deps.state.setActiveAttemptId(chunk.attemptId, chunk.messageId);
    }
    deps.state.setReasoningStreaming(false);
    deps.state.appendDelta(chunk.delta);
  }

  function onStreamStart(
    conversationId: string,
    attemptId: string,
    messageId: string,
  ) {
    turnIdByConversation.set(conversationId, attemptId);
    if (conversationId !== deps.env.getActiveConversationId()) return;
    deps.state.setActiveAttemptId(attemptId, messageId);
  }

  function onUserMessageConfirmed(
    userMessageId: string,
    userMessageContent: string,
  ) {
    deps.state.confirmUserMessage(userMessageId, userMessageContent);
  }

  /**
   * `agent-tool-call` 事件只负责实时渲染，不作为事实源（I38d §2.1）。
   * 事件不带 `messageId`，因此按 `turnId` 归到当前流的助手消息；
   * 旧轮（`turnId` 与当前 attempt 不符）的迟到事件直接丢弃。
   */
  function onAgentToolCall(call: AgentToolCall) {
    const activeTurnId = deps.env.getActiveAttemptId();
    if (!activeTurnId || call.turnId !== activeTurnId) return;
    const messageId = deps.env.getStreamingMessageId();
    if (!messageId) return;
    deps.state.applyToolCall(messageId, call);
  }

  /**
   * `session-state-changed` 是 phase 的唯一来源（I38d §2.3）。
   * 按会话记录，切会话不丢后台 turn 的状态（C5.2）；只有当前会话才驱动 UI。
   */
  function onSessionState(payload: SessionStateChangedPayload) {
    const convId = payload.conversationId;
    const next = stateToPhase(payload.state);
    if (next === "idle") {
      phaseByConversation.delete(convId);
      turnIdByConversation.delete(convId);
    } else {
      phaseByConversation.set(convId, next);
      turnIdByConversation.set(convId, payload.turnId);
    }

    if (convId !== deps.env.getActiveConversationId()) return;
    setPhase(next);

    if (next === "idle" && deps.env.getIsStreaming()) {
      settleActiveToolCalls();
      deps.state.finishStream();
      return;
    }
    if (next === "error" && deps.env.getIsStreaming()) {
      settleActiveToolCalls();
      deps.state.cancelStreaming();
      return;
    }
    if (isRunning(next) && !deps.env.getIsStreaming()) {
      deps.state.resumeStream(payload.turnId, deps.env.getStreamingMessageId());
    }
  }

  /** 切会话：不取消后台 turn，只把本地流式视图换成目标会话的状态（A4）。 */
  function onConversationSwitch(conversationId: string) {
    const next = phaseByConversation.get(conversationId) ?? "idle";
    setPhase(next);
    deps.state.cancelStreaming();
    if (isRunning(next)) {
      const turnId = turnIdByConversation.get(conversationId);
      if (turnId) deps.state.resumeStream(turnId, null);
    }
  }

  return {
    send,
    cancel,
    handleDeltaEvent,
    onTextDelta,
    onStreamStart,
    onUserMessageConfirmed,
    onAgentToolCall,
    onSessionState,
    onConversationSwitch,
    getSendInProgress: () => sendInProgress,
    getPhase: () => phase,
    getActiveConversationId: () => deps.env.getActiveConversationId(),
  };
}
