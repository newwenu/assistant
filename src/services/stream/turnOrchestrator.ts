import type {
  Message,
  Conversation,
  AgentToolCall,
  ChatDeltaPayload,
  SendChatMessageArgs,
  SessionStateChangedPayload,
} from "../../chat/core/chatTypes";
import type { ConversationRoute } from "../../chat/core/message/conversationLink";
import { createOptimisticMessage } from "../../chat/core/stream/optimisticMessage";
import { contextUsageFromTokenUsage } from "../../chat/core/chatUtils";
import {
  toReasoningLevel,
  type ReasoningChoice,
} from "../../chat/core/reasoning";
import { ChatErrorCode, toChatError } from "../../chat/core/chatErrors";
import type { Provider } from "../../settings/provider/types";
import { createLogger } from "../system/logger";
import type { StreamTextChunk } from "./transport";
import type { StreamStatePort, TurnKind, TurnStrategy } from "./turnStrategy";
import { stateToPhase, type ChatPhase } from "./phase";
import { fallbackTitleFromUserMessage } from "./title";

export type { ChatPhase } from "./phase";
export { fallbackTitleFromUserMessage } from "./title";

const log = createLogger("turnOrchestrator");

export interface TurnEnvPort {
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

export interface TurnOrchestratorDeps {
  state: StreamStatePort;
  env: TurnEnvPort;
  /** 按链路注入的策略；编排骨架不感知具体链路语义（I38d §3.7）。 */
  strategies: Record<TurnKind, TurnStrategy>;
  /** 发送命令分派：由组合层按 `kind` 选具体命令，骨架不做链路判断。 */
  dispatchSend: (
    conversationId: string,
    args: SendChatMessageArgs,
    kind: TurnKind,
  ) => Promise<void>;
  /** 取消命令分派：同样按链路由组合层选择。 */
  cancelTurn: (conversationId: string) => Promise<void>;
  listProviders: () => Promise<Provider[]>;
  createConversation: (
    providerId: string,
    modelId: string,
    modeId?: string | null,
  ) => Promise<string>;
  loadConversations: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ) => Promise<void>;
}

export interface TurnOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  onTextDelta: (chunk: StreamTextChunk) => void;
  onStreamStart: (
    conversationId: string,
    attemptId: string,
    messageId: string,
    turnIndex: number | null,
    iteration: number | null,
  ) => void;
  onUserMessageConfirmed: (
    userMessageId: string,
    userMessageContent: string,
    turnIndex: number | null,
  ) => void;
  onAgentToolCall: (call: AgentToolCall) => void;
  onSessionState: (payload: SessionStateChangedPayload) => void;
  /**
   * 切换会话：**不取消**后台 turn，只把本地流式视图换成目标会话的 phase（C5.2 / A4）。
   * 目标会话仍在运行时恢复流式视图，`messageId` 由后续 delta 补全。
   */
  onConversationSwitch: (conversationId: string) => void;
  /**
   * 补拉快照回填：会话仍有挂起的工具调用（当前只有 `ask_user`）。
   *
   * 窗口重载后 `phaseByConversation` / `turnIdByConversation` 是空的，
   * 只靠事件无法得知轮次仍在进行，切回时会被判成 `idle` 而不恢复流式视图；
   * 快照是唯一的兜底来源，用它恢复 phase 与段表，否则作答后的事件会落到新开的占位气泡。
   */
  onPendingToolCalls: (
    conversationId: string,
    turnId: string,
    messageId: string | null,
    iteration: number,
  ) => void;
  getSendInProgress: () => boolean;
  getPhase: () => ChatPhase;
  getActiveConversationId: () => string | null;
}

export function createTurnOrchestrator(
  deps: TurnOrchestratorDeps,
): TurnOrchestrator {
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
   * 策略选择的**唯一**判据点：会话 `modeId` 解析出的链路。
   * 其余位置一律通过策略间接分叉，禁止再写 `if (route.link === "agent")`（I38d §3.7）。
   */
  function strategyFor(conversationId: string | null): TurnStrategy {
    if (!conversationId) return deps.strategies.chat;
    const route = deps.env.getConversationRoute(conversationId);
    return route.link === "agent"
      ? deps.strategies.agent
      : deps.strategies.chat;
  }

  function beginTurn(
    conversationId: string | null,
    messages: Message[],
    assistantMsgId: string,
  ) {
    deps.state.beginSend(messages, assistantMsgId);
    strategyFor(conversationId).onBeginSend(assistantMsgId);
  }

  /**
   * 切回运行中的会话时 `messageId` 未知，由首个增量补全。
   * 仅在 `streamingMessageId` 为空（恢复场景）时执行，避免覆盖已有绑定。
   */
  function resolveStreamingMessageId(attemptId: string, messageId: string) {
    if (deps.env.getStreamingMessageId() !== null) return;
    deps.state.setActiveAttemptId(attemptId, messageId);
  }

  async function send(content: string) {
    if (sendInProgress) return;
    sendInProgress = true;

    try {
      if (deps.env.getIsStreaming()) {
        const prevConvId = deps.env.getActiveConversationId();
        if (prevConvId) {
          await deps
            .cancelTurn(prevConvId)
            .catch((e) => log.warn("cancelTurn failed in send", e));
        }
        strategyFor(prevConvId).onCancel();
        deps.state.clearActiveAttemptId();
      }

      let convId = deps.env.getActiveConversationId();
      if (!convId) {
        const provList = await deps.listProviders();
        if (provList.length === 0) {
          const errConvId = `err-${Date.now()}`;
          const userMsg: Message = createOptimisticMessage({
            id: `temp-user-${Date.now()}`,
            conversationId: errConvId,
            role: "user",
            content,
            timestamp: Date.now(),
            parentId: null,
          });
          const assistantMsgId = `temp-assistant-${Date.now()}`;
          const assistantMsg: Message = createOptimisticMessage({
            id: assistantMsgId,
            conversationId: errConvId,
            role: "assistant",
            content: "",
            timestamp: Date.now() + 1,
            parentId: null,
          });
          beginTurn(null, [userMsg, assistantMsg], assistantMsgId);
          deps.state.errorStream({
            code: ChatErrorCode.ApiKeyMissing,
            retryAfterMs: null,
            retryable: false,
          });
          return;
        }
        const p = provList[0];
        const newId = await deps.createConversation(
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

      // 身份指示与落库后保持一致（I43 §4.1）：乐观消息必须带 `modeId` / `providerId` /
      // `modelId`，否则流式首屏与落库回读不同源、视觉会跳。读取因此前移到消息构造之前，
      // 后续复用同一组局部变量，禁止重复调用 env 端口。
      const route = deps.env.getConversationRoute(convId);
      const selectedProviderId = deps.env.getSelectedProviderId();
      const selectedModelId = deps.env.getSelectedModelId();

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
        const assistantMsg: Message = createOptimisticMessage({
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          timestamp: Date.now(),
          parentId: userParentId,
          providerId: selectedProviderId,
          modelId: selectedModelId,
          modeId: route.modeId,
        });
        optimisticMessages.push(assistantMsg);
        beginTurn(convId, optimisticMessages, assistantMsgId);
      } else {
        const userMsg: Message = createOptimisticMessage({
          id: `temp-user-${Date.now()}`,
          conversationId: convId,
          role: "user",
          content,
          timestamp: Date.now(),
          parentId: userParentId,
        });
        const assistantMsgId = `temp-assistant-${Date.now()}`;
        const assistantMsg: Message = createOptimisticMessage({
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          timestamp: Date.now() + 1,
          parentId: userMsg.id,
          providerId: selectedProviderId,
          modelId: selectedModelId,
          modeId: route.modeId,
        });
        optimisticMessages.push(userMsg, assistantMsg);
        beginTurn(convId, optimisticMessages, assistantMsgId);
      }

      const convs = deps.env.getConversations();
      const conv = convs.find((c) => c.id === convId);
      if (conv && conv.titleSource === "placeholder") {
        const fbTitle = fallbackTitleFromUserMessage(content);
        void deps.renameConversation(convId, fbTitle, "fallback");
      }

      const kind = strategyFor(convId).kind;
      try {
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
        await deps.dispatchSend(convId, args, kind);
      } catch (e) {
        log.error("sendMessage failed", e);
        deps.state.errorStream(
          toChatError(e, {
            command:
              kind === "agent" ? "send_agent_message" : "send_chat_message",
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
      await deps.cancelTurn(convId);
    } catch (e) {
      log.error("cancel failed", e);
    }
    strategyFor(convId).onCancel();
    turnIdByConversation.delete(convId);
    phaseByConversation.delete(convId);
    // 压缩不是 turn（I39 §1.2）：其流式占位气泡是临时摘要行，取消后由后端落库的
    // `interrupted` 摘要行刷新呈现，不在该气泡上写 `Cancelled` 错误（I41 §1）。
    const isCompactionStream =
      streamingMsgId !== null &&
      deps.env
        .getMessages()
        .some((m) => m.id === streamingMsgId && m.role === "compact_summary");
    if (streamingMsgId && !isCompactionStream) {
      deps.state.markMessageInterrupted(streamingMsgId);
    }
    setPhase("idle");
  }

  function handleDeltaEvent(payload: ChatDeltaPayload) {
    if (payload.conversationId !== deps.env.getActiveConversationId()) return;

    const activeAttemptId = deps.env.getActiveAttemptId();
    if (activeAttemptId === null || payload.attemptId !== activeAttemptId) {
      return;
    }

    const strategy = strategyFor(payload.conversationId);
    resolveStreamingMessageId(payload.attemptId, payload.messageId);

    if (payload.error) {
      strategy.onDeltaError(toChatError(payload.error));
      return;
    }

    if (payload.deltaRetry) {
      deps.state.setRetryStatus(payload.deltaRetry);
      return;
    }

    if (payload.deltaReasoning) {
      strategy.onReasoningDelta(payload);
    }

    if (payload.usage) {
      deps.state.updateContextUsage(contextUsageFromTokenUsage(payload.usage));
    }

    if (payload.finishReason) {
      strategy.onFinishReason(payload);
    }
  }

  /**
   * 文本增量按会话路由：后台会话的增量不落到当前会话的流式缓冲（C5.2）。
   * phase 由 `session-state-changed` 驱动，这里不再自行推导（C5.3）。
   */
  function onTextDelta(chunk: StreamTextChunk) {
    if (chunk.conversationId !== deps.env.getActiveConversationId()) return;
    const activeAttemptId = deps.env.getActiveAttemptId();
    if (activeAttemptId === null || chunk.attemptId !== activeAttemptId) {
      return;
    }
    const strategy = strategyFor(chunk.conversationId);
    resolveStreamingMessageId(chunk.attemptId, chunk.messageId);
    strategy.onTextDelta(chunk);
  }

  function onStreamStart(
    conversationId: string,
    attemptId: string,
    messageId: string,
    turnIndex: number | null,
    iteration: number | null,
  ) {
    turnIdByConversation.set(conversationId, attemptId);
    // 坐标是 wire 事实（I46 §3.2）：本轮 turn 序号落进会话无关的瞬时字段，
    // 供后续迭代与占位消息赋坐标，不从相邻关系推算。
    deps.state.setCurrentTurnIndex(turnIndex);
    if (conversationId !== deps.env.getActiveConversationId()) return;
    const previousId = deps.env.getStreamingMessageId();
    deps.state.setActiveAttemptId(attemptId, messageId);
    if (turnIndex !== null && iteration !== null) {
      deps.state.setMessageCoords(messageId, turnIndex, iteration);
    }
    // 占位消息换成真实行 id 时，策略侧（agent 段表）需要同步改名（§3.7）
    if (previousId !== null && previousId !== messageId) {
      strategyFor(conversationId).onMessageIdResolved(previousId, messageId);
    }
  }

  function onUserMessageConfirmed(
    userMessageId: string,
    userMessageContent: string,
    turnIndex: number | null,
  ) {
    deps.state.confirmUserMessage(userMessageId, userMessageContent, turnIndex);
  }

  /**
   * `agent-tool-call` 事件只负责实时渲染，不作为事实源（I38d §2.1）。
   * 事件不带 `messageId`，因此按 `turnId` 校验轮次、按 `iteration` 归到发起该调用的那条消息
   * （气泡边界 = 迭代，§3.1.1）；旧轮（`turnId` 与当前 attempt 不符）的迟到事件直接丢弃。
   */
  function onAgentToolCall(call: AgentToolCall) {
    const activeTurnId = deps.env.getActiveAttemptId();
    if (!activeTurnId || call.turnId !== activeTurnId) return;
    strategyFor(call.conversationId).onToolCall(call);
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
      strategyFor(convId).onTurnIdle();
      return;
    }
    if (next === "error" && deps.env.getIsStreaming()) {
      strategyFor(convId).onTurnError();
      return;
    }
    if (isRunning(next) && !deps.env.getIsStreaming()) {
      const strategy = strategyFor(convId);
      const streamingMessageId = deps.env.getStreamingMessageId();
      strategy.onResume(streamingMessageId);
      deps.state.resumeStream(payload.turnId, streamingMessageId);
    }
  }

  /** 切会话：不取消后台 turn，只把本地流式视图换成目标会话的状态（A4）。 */
  function onConversationSwitch(conversationId: string) {
    const next = phaseByConversation.get(conversationId) ?? "idle";
    setPhase(next);
    strategyFor(conversationId).onResume(null);
    deps.state.cancelStreaming();
    if (isRunning(next)) {
      const turnId = turnIdByConversation.get(conversationId);
      if (turnId) deps.state.resumeStream(turnId, null);
    }
  }

  /**
   * 补拉快照回填：只补「轮次仍在进行」这一事实，卡片本身由调用方回挂到消息上。
   * `messageId` 为 `null`（历史没加载到发起行）时退回 `onConversationSwitch` 的未知段语义。
   */
  function onPendingToolCalls(
    conversationId: string,
    turnId: string,
    messageId: string | null,
    iteration: number,
  ) {
    const next = phaseByConversation.get(conversationId) ?? "tool_executing";
    phaseByConversation.set(conversationId, next);
    turnIdByConversation.set(conversationId, turnId);
    if (conversationId !== deps.env.getActiveConversationId()) return;
    setPhase(next);
    strategyFor(conversationId).onResume(messageId, iteration);
    deps.state.resumeStream(turnId, messageId);
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
    onPendingToolCalls,
    getSendInProgress: () => sendInProgress,
    getPhase: () => phase,
    getActiveConversationId: () => deps.env.getActiveConversationId(),
  };
}
