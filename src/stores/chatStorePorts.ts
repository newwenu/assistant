import type {
  ChatStatePort,
  ChatEnvPort,
  ChatOrchestrator,
  ChatOrchestratorDeps,
} from "../services/chatOrchestrator";
import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
} from "../chat/core/chatTypes";
import type { RetryNotification } from "../chat/core/chatTypes";
import { type ChatError } from "../chat/core/chatErrors";
import { chatService } from "../services/chatService";
import { createChatOrchestrator } from "../services/chatOrchestrator";
import { useChatStreamStore } from "./chatStreamStore";
import { useAgentModeStore } from "./agentModeStore";
import { createLogger } from "@/services/logger";
import {
  mergeStreamingContent,
  fetchTokenUsage,
  type FetchTokenUsageDeps,
} from "../chat/core/chatUtils";

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
}

export function createStatePort(
  set: (partial: Partial<StatePortWritable>) => void,
  get: () => StatePortReadable,
  fetchTokenUsageDeps: FetchTokenUsageDeps,
): ChatStatePort {
  return {
    beginSend: (messages: Message[], assistantMsgId: string) => {
      useChatStreamStore.getState().beginStream(assistantMsgId);
      set({
        messages,
        contextUsage: { usedTokens: 0, maxTokens: 128_000, percentage: 0 },
      });
    },
    cancelStreaming: () => {
      const stream = useChatStreamStore.getState();
      useChatStreamStore.getState().cancelStreaming();
      set({
        messages: mergeStreamingContent(
          get().messages,
          stream.streamingMessageId,
          stream.streamingContent,
        ),
      });
    },
    setActiveAttemptId: (attemptId: string, realMessageId: string) => {
      const state = get();
      const oldStreamingMessageId =
        useChatStreamStore.getState().streamingMessageId;
      useChatStreamStore
        .getState()
        .setActiveAttemptId(attemptId, realMessageId);
      set({
        messages: state.messages.map((m) =>
          m.id === oldStreamingMessageId ? { ...m, id: realMessageId } : m,
        ),
      });
    },
    clearActiveAttemptId: () => {
      useChatStreamStore.getState().clearActiveAttemptId();
    },
    appendDelta: (delta: string) => {
      useChatStreamStore.getState().appendDelta(delta);
    },
    setReasoningStreaming: (streaming: boolean) => {
      useChatStreamStore.getState().setReasoningStreaming(streaming);
    },
    appendReasoningDelta: (messageId: string, delta: string) => {
      const state = get();
      set({
        messages: state.messages.map((m) =>
          m.id === messageId
            ? { ...m, reasoning: (m.reasoning || "") + delta }
            : m,
        ),
      });
    },
    appendToolCallDelta: (
      messageId: string,
      toolCallId: string | undefined,
      toolCallName: string | undefined,
      toolCallArguments: string | undefined,
    ) => {
      const state = get();
      set({
        messages: state.messages.map((m) => {
          if (m.id !== messageId) return m;
          const toolCalls = [...(m.toolCalls || [])];
          if (toolCallId) {
            toolCalls.push({
              id: toolCallId,
              name: toolCallName || "",
              arguments: toolCallArguments || "",
            });
          } else if (toolCalls.length > 0) {
            const last = toolCalls[toolCalls.length - 1];
            if (toolCallName) last.name += toolCallName;
            if (toolCallArguments) last.arguments += toolCallArguments;
          }
          return { ...m, toolCalls };
        }),
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
            usedTokens: usage.promptTokens,
            maxTokens: usage.contextWindowTokens || 128_000,
            percentage:
              usage.contextWindowTokens && usage.contextWindowTokens > 0
                ? usage.promptTokens / usage.contextWindowTokens
                : usage.promptTokens / 128_000,
          }
        : state.contextUsage;

      const msgTokens = usage
        ? usage.promptTokens + usage.completionTokens
        : null;

      useChatStreamStore.getState().cancelStreaming();
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

      if (convId) {
        chatService.incrementGeneration(convId).catch(() => {});
      }

      if (convId && !usage) {
        const conv = state.conversations.find((c) => c.id === convId);
        fetchTokenUsage(
          convId,
          conv?.modelId ?? "",
          conv?.providerId ?? null,
          fetchTokenUsageDeps,
        );
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
}

export function createEnvPort(
  get: () => EnvPortReadable,
  set: (partial: Partial<EnvPortWritable>) => void,
  getSelectedProviderId: () => string | null,
  getSelectedModelId: () => string,
): ChatEnvPort {
  return {
    getActiveConversationId: () => get().currentConversationId,
    getIsStreaming: () => useChatStreamStore.getState().isStreaming,
    getStreamingMessageId: () =>
      useChatStreamStore.getState().streamingMessageId,
    getActiveAttemptId: () => useChatStreamStore.getState().activeAttemptId,
    getMessages: () => get().messages,
    getConversations: () => get().conversations,
    getSelectedModeId: () => useAgentModeStore.getState().selectedModeId,
    getSelectedProviderId,
    getSelectedModelId,
    getPendingBranchPoint: () => get().pendingBranchPoint,
    clearPendingBranchPoint: () => set({ pendingBranchPoint: null }),
  };
}

export function createOrchestrator(
  statePort: ChatStatePort,
  envPort: ChatEnvPort,
  deps: Omit<ChatOrchestratorDeps, "state" | "env">,
): ChatOrchestrator {
  return createChatOrchestrator({
    state: statePort,
    env: envPort,
    ...deps,
  });
}
