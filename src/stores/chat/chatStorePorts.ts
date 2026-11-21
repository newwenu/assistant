import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
} from "../../chat/core/chatTypes";
import type { RetryNotification } from "../../chat/core/chatTypes";
import { type ChatError } from "../../chat/core/chatErrors";
import { chatService } from "../../services/chat/chatService";
import { providerService } from "../../services/providerService";
import {
  createTurnOrchestrator,
  type TurnEnvPort,
  type TurnOrchestrator,
  type TurnOrchestratorDeps,
} from "../../services/stream/turnOrchestrator";
import type {
  StreamStatePort,
  TurnStrategy,
} from "../../services/stream/turnStrategy";
import { createChatTurnStrategy } from "../../services/stream/strategies/chatTurn";
import { createAgentTurnStrategy } from "../../services/stream/strategies/agentTurn";
import {
  createAgentSegments,
  type AgentSegments,
} from "../agent/agentSegments";
import { useObservationStore } from "../agent/observation/store";
import { useChatStreamStore } from "./chatStreamStore";
import { useAgentModeStore } from "../agent/agentModeStore";
import {
  resolveReasoningChoice,
  type ReasoningChoice,
} from "../../chat/core/reasoning";
import {
  resolveConversationRoute,
  type ConversationRoute,
} from "../../chat/core/message/conversationLink";
import { createLogger } from "@/services/system/logger";
import {
  mergeStreamingContent,
  fetchTokenUsage,
  contextUsageFromTokenUsage,
  type FetchTokenUsageDeps,
} from "../../chat/core/chatUtils";

const log = createLogger("chatStorePorts");

interface StatePortWritable {
  messages: Message[];
  contextUsage: ContextUsage;
}

interface EnvPortWritable {
  pendingBranchPoint: string | null;
}

interface StatePortReadable {
  messages: Message[];
  currentConversationId: string | null;
  contextUsage: ContextUsage;
  conversations: Conversation[];
}

interface EnvPortReadable {
  currentConversationId: string | null;
  messages: Message[];
  conversations: Conversation[];
  pendingBranchPoint: string | null;
  reasoningChoice: ReasoningChoice;
}

export interface ChatStatePorts {
  /** 通用视图状态端口：只操作「当前正在流的那一条消息」。 */
  port: StreamStatePort;
  /** agent 私有段表（迭代 → 段消息 id），由 `agentTurn` 策略独占使用（I38d §3.7）。 */
  segments: AgentSegments;
}

/**
 * 视图状态端口实现。
 *
 * 这里只保留两条链路都要的能力（占位消息、流式缓冲、合并落盘、错误挂载）；
 * 迭代分段 / 工具归属 / 段表等 agent 语义已下沉到 `stores/agent/agentSegments.ts`。
 */
export function createStatePort(
  set: (partial: Partial<StatePortWritable>) => void,
  get: () => StatePortReadable,
  fetchTokenUsageDeps: FetchTokenUsageDeps,
): ChatStatePorts {
  const segments = createAgentSegments(set, get);

  const port: StreamStatePort = {
    beginSend: (messages: Message[], assistantMsgId: string) => {
      useChatStreamStore.getState().beginStream(assistantMsgId);
      // 发送不清零占用：占用反映最近一次请求，与本次发送无关
      set({ messages });
    },
    cancelStreaming: () => {
      const stream = useChatStreamStore.getState();
      set({
        messages: mergeStreamingContent(
          get().messages,
          stream.streamingMessageId,
          stream.streamingContent,
        ),
      });
      useChatStreamStore.getState().cancelStreaming();
    },
    setActiveAttemptId: (attemptId: string, realMessageId: string) => {
      const oldStreamingMessageId =
        useChatStreamStore.getState().streamingMessageId;
      useChatStreamStore
        .getState()
        .setActiveAttemptId(attemptId, realMessageId);
      if (
        oldStreamingMessageId === null ||
        oldStreamingMessageId === realMessageId
      ) {
        return;
      }
      const state = get();
      set({
        messages: state.messages.map((m) =>
          m.id === oldStreamingMessageId ? { ...m, id: realMessageId } : m,
        ),
      });
    },
    clearActiveAttemptId: () => {
      useChatStreamStore.getState().clearActiveAttemptId();
    },
    resumeStream: (attemptId: string, messageId: string | null) => {
      useChatStreamStore.getState().resumeStream(attemptId, messageId);
    },
    setCurrentTurnIndex: (turnIndex: number | null) => {
      useChatStreamStore.getState().setCurrentTurnIndex(turnIndex);
    },
    setMessageCoords: (
      messageId: string,
      turnIndex: number,
      iteration: number,
    ) => {
      const state = get();
      if (!state.messages.some((m) => m.id === messageId)) return;
      set({
        messages: state.messages.map((m) =>
          m.id === messageId ? { ...m, turnIndex, iteration } : m,
        ),
      });
    },
    confirmUserMessage: (
      userMessageId: string,
      userMessageContent: string,
      turnIndex: number | null,
    ) => {
      const state = get();
      const tempUserMsg = state.messages.find(
        (m) => m.role === "user" && m.id.startsWith("temp-user-"),
      );
      if (!tempUserMsg) return;
      set({
        messages: state.messages.map((m) =>
          m.id === tempUserMsg.id
            ? {
                ...m,
                id: userMessageId,
                content: userMessageContent,
                turnIndex: turnIndex ?? m.turnIndex,
              }
            : m,
        ),
      });
    },
    appendDelta: (delta: string) => {
      useChatStreamStore.getState().appendDelta(delta);
    },
    setReasoningStreaming: (streaming: boolean) => {
      useChatStreamStore.getState().setReasoningStreaming(streaming);
    },
    appendReasoningDelta: (delta: string) => {
      const messageId = useChatStreamStore.getState().streamingMessageId;
      if (!messageId) return;
      const state = get();
      set({
        messages: state.messages.map((m) =>
          m.id === messageId
            ? { ...m, reasoning: (m.reasoning || "") + delta }
            : m,
        ),
      });
    },
    updateContextUsage: (usage: ContextUsage) => {
      set({ contextUsage: usage });
    },
    finishStream: (usage?: TokenUsage) => {
      const stream = useChatStreamStore.getState();
      if (!stream.isStreaming) return;
      const state = get();
      const convId = state.currentConversationId;
      const newContextUsage = usage
        ? {
            ...contextUsageFromTokenUsage(usage),
            cumulativeTokens: state.contextUsage.cumulativeTokens,
          }
        : state.contextUsage;

      const msgTokens = usage
        ? usage.promptTokens + usage.completionTokens
        : null;

      set({
        contextUsage: newContextUsage,
        messages: mergeStreamingContent(
          state.messages,
          stream.streamingMessageId,
          stream.streamingContent,
        ).map((m) =>
          m.id === stream.streamingMessageId &&
          msgTokens !== null &&
          msgTokens > 0
            ? { ...m, tokens: msgTokens }
            : m,
        ),
      });
      useChatStreamStore.getState().cancelStreaming();

      if (convId) {
        chatService
          .incrementGeneration(convId)
          .catch((e: unknown) =>
            log.warn("incrementGeneration failed", { convId, error: e }),
          );
      }

      if (convId && !usage) {
        fetchTokenUsage(convId, fetchTokenUsageDeps);
      }
    },
    setErrorOnMessage: (messageId: string, error: ChatError) => {
      const state = get();
      const existing = state.messages.find((m) => m.id === messageId);
      if (!existing || existing.error) return;
      const retryHint =
        error.retryAfterMs != null && error.retryAfterMs > 0
          ? `(retry after ${Math.ceil(error.retryAfterMs / 1000)}s)`
          : "";
      const displayMessage = retryHint
        ? `${error.message ?? ""} ${retryHint}`
        : (error.message ?? error.code);
      set({
        messages: state.messages.map((m) =>
          m.id === messageId ? { ...m, error } : m,
        ),
      });
      log.warn(
        `chat-error code=${error.code} retryable=${error.retryable} message=${displayMessage}`,
      );
    },
    markMessageInterrupted: (messageId: string) => {
      const state = get();
      if (!state.messages.some((m) => m.id === messageId)) return;
      set({
        messages: state.messages.map((m) =>
          m.id === messageId ? { ...m, status: "interrupted" } : m,
        ),
      });
    },
    errorStream: (error: ChatError) => {
      const stream = useChatStreamStore.getState();
      if (!stream.streamingMessageId) return;
      const merged = mergeStreamingContent(
        get().messages,
        stream.streamingMessageId,
        stream.streamingContent,
      );
      const existing = merged.find((m) => m.id === stream.streamingMessageId);
      if (existing?.error) {
        useChatStreamStore.getState().cancelStreaming();
        return;
      }
      set({
        messages: merged.map((m) =>
          m.id === stream.streamingMessageId ? { ...m, error } : m,
        ),
      });
      useChatStreamStore.getState().cancelStreaming();
      const retryHint2 =
        error.retryAfterMs != null && error.retryAfterMs > 0
          ? `(retry after ${Math.ceil(error.retryAfterMs / 1000)}s)`
          : "";
      const displayMessage2 = retryHint2
        ? `${error.message ?? ""} ${retryHint2}`
        : (error.message ?? error.code);
      log.warn(
        `chat-error code=${error.code} retryable=${error.retryable} message=${displayMessage2}`,
      );
    },
    setRetryStatus: (retry: RetryNotification | null) => {
      useChatStreamStore.getState().setRetryStatus(retry);
    },
  };

  return { port, segments };
}

export function createEnvPort(
  get: () => EnvPortReadable,
  set: (partial: Partial<EnvPortWritable>) => void,
  getSelectedProviderId: () => string | null,
  getSelectedModelId: () => string,
): TurnEnvPort {
  /**
   * 链路判据取自会话自身：`ConversationDto.modeId` → `agent_modes` 行 → `kind`。
   * 解析逻辑集中在 `resolveConversationRoute`，避免多处口径漂移（I38d §4.8）。
   * agent 链路的每轮工具配置取「会话持久值 ?? 模式初始值」。
   */
  function getConversationRoute(conversationId: string): ConversationRoute {
    const conv = get().conversations.find((c) => c.id === conversationId);
    return resolveConversationRoute(
      conv?.modeId ?? null,
      useAgentModeStore.getState().modes ?? [],
      conv?.toolConfig ?? null,
    );
  }

  return {
    getActiveConversationId: () => get().currentConversationId,
    getIsStreaming: () => useChatStreamStore.getState().isStreaming,
    getStreamingMessageId: () =>
      useChatStreamStore.getState().streamingMessageId,
    getActiveAttemptId: () => useChatStreamStore.getState().activeAttemptId,
    getMessages: () => get().messages,
    getConversations: () => get().conversations,
    getConversationRoute,
    getReasoningChoice: () => {
      const { modes, selectedModeId } = useAgentModeStore.getState();
      return resolveReasoningChoice(
        get().reasoningChoice,
        modes.find((m) => m.id === selectedModeId),
      );
    },
    getSelectedProviderId,
    getSelectedModelId,
    getPendingBranchPoint: () => get().pendingBranchPoint,
    clearPendingBranchPoint: () => set({ pendingBranchPoint: null }),
  };
}

export function createOrchestrator(
  statePort: StreamStatePort,
  envPort: TurnEnvPort,
  segments: AgentSegments,
  deps: Pick<
    TurnOrchestratorDeps,
    "loadConversations" | "switchConversation" | "renameConversation"
  >,
): TurnOrchestrator {
  const strategies: Record<TurnStrategy["kind"], TurnStrategy> = {
    chat: createChatTurnStrategy(statePort),
    agent: createAgentTurnStrategy(statePort, segments),
  };

  return createTurnOrchestrator({
    state: statePort,
    env: envPort,
    strategies,
    // 观测端口只暴露写方法：编排层读不到观测数据（O6 / O10）。
    observation: {
      applyNodeEvent: (event) =>
        useObservationStore.getState().applyNodeEvent(event),
      applyDeltaSeq: (conversationId, turnId, seq, iteration) =>
        useObservationStore
          .getState()
          .applyDeltaSeq(conversationId, turnId, seq, iteration),
      applyToolCall: (call) =>
        useObservationStore.getState().applyToolCall(call),
      applySessionState: (conversationId, turnId, state) =>
        useObservationStore
          .getState()
          .applySessionState(conversationId, turnId, state),
    },
    // 命令分派是链路判断的第二个（也是最后一个）落点：命令与服务必须同链
    dispatchSend: (conversationId, args, kind) =>
      kind === "agent"
        ? chatService.sendAgentMessage(conversationId, args)
        : chatService.sendChatMessage(conversationId, args),
    cancelTurn: (conversationId) =>
      envPort.getConversationRoute(conversationId).link === "agent"
        ? chatService.cancelAgent(conversationId)
        : chatService.cancelChat(conversationId),
    listProviders: () => providerService.listProviders(),
    createConversation: (providerId, modelId, modeId) =>
      chatService.createConversation(providerId, modelId, modeId),
    ...deps,
  });
}
