import { create } from "zustand";
import { chatService } from "../services/chatService";
import {
  createChatOrchestrator,
  type ChatStatePort,
  type ChatEnvPort,
  type ChatOrchestrator,
  type RetryNotification,
} from "../services/chatOrchestrator";
import {
  MESSAGES_PAGE_SIZE,
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../config/constants";
import { type ChatError, formatRetryAfter } from "../chat/chatErrors";
import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
} from "../chat/chatTypes";
export type {
  Message,
  ToolCall,
  Conversation,
  TokenUsage,
  ContextUsage,
} from "../chat/chatTypes";
import { PersistQueue } from "../utils/persistQueue";
import { useAgentModeStore } from "./agentModeStore";
import { useProviderStore, type ModelConfig } from "./providerStore";

const persistQueue = new PersistQueue();

function resolveContextWindowTokens(
  modelId: string,
  providerId: string | null,
): number | undefined {
  if (!providerId) return undefined;
  const provider = useProviderStore
    .getState()
    .providers.find((p) => p.id === providerId);
  if (!provider) return undefined;
  try {
    const models = JSON.parse(provider.models) as ModelConfig[];
    const model = models.find((m) => m.id === modelId);
    if (model && model.context_window && model.context_window > 0) {
      return model.context_window;
    }
  } catch {
    // ignore parse errors
  }
  return undefined;
}

interface ChatState {
  conversations: Conversation[];
  currentConversationId: string | null;
  messages: Message[];
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  isReasoningStreaming: boolean;
  hasMoreMessages: boolean;
  isLoadingMoreMessages: boolean;
  isLoadingConversations: boolean;
  totalMessageCount: number;
  contextUsage: ContextUsage;
  isFull: boolean;
  compactSummary: string | null;
  compactError: string | null;
  retryStatus: RetryNotification | null;

  loadConversations: (silent?: boolean) => Promise<void>;
  createConversation: (providerId: string, modelId: string) => Promise<string>;
  ensureCurrentConversation: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  renameConversation: (
    id: string,
    title: string,
    titleSource?: string,
  ) => Promise<void>;
  updateModel: (id: string, modelId: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelStream: () => Promise<void>;
  appendDelta: (delta: string) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  deleteMessage: (id: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  regenerateMessage: (messageId: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  duplicateConversation: (id: string) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  purgeAll: () => Promise<void>;
  manualCompact: () => Promise<void>;
  unlockConversation: () => Promise<void>;
  clearCompactError: () => void;
}

let _orchestrator: ChatOrchestrator | null = null;

export const useChatStore = create<ChatState>((set, get) => {
  const statePort: ChatStatePort = {
    beginSend: (messages: Message[], assistantMsgId: string) => {
      set({
        messages,
        isStreaming: true,
        streamingContent: "",
        streamingMessageId: assistantMsgId,
        contextUsage: { used_tokens: 0, max_tokens: 128_000, percentage: 0 },
        retryStatus: null,
      });
    },
    cancelStreaming: () => {
      set({
        isStreaming: false,
        streamingContent: "",
        streamingMessageId: null,
        isReasoningStreaming: false,
        retryStatus: null,
      });
    },
    appendDelta: (delta: string) => {
      const state = get();
      const newContent = state.streamingContent + delta;
      if (import.meta.env.DEV) {
        console.debug(
          `[appendDelta] +${delta.length}ch prev=${state.streamingContent.length} new=${newContent.length} snippet=${JSON.stringify(delta.slice(0, 40))}`,
        );
      }
      set({ streamingContent: newContent });
    },
    setReasoningStreaming: (streaming: boolean) => {
      set({ isReasoningStreaming: streaming });
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
      const state = get();
      if (!state.isStreaming) return;
      const convId = state.currentConversationId;
      const newContextUsage = usage
        ? {
            used_tokens: usage.prompt_tokens,
            max_tokens: usage.context_window_tokens || 128_000,
            percentage:
              usage.context_window_tokens && usage.context_window_tokens > 0
                ? usage.prompt_tokens / usage.context_window_tokens
                : usage.prompt_tokens / 128_000,
          }
        : state.contextUsage;

      const msgTokens = usage
        ? usage.prompt_tokens + usage.completion_tokens
        : null;

      set({
        isStreaming: false,
        streamingContent: "",
        streamingMessageId: null,
        isReasoningStreaming: false,
        contextUsage: newContextUsage,
        messages: state.streamingMessageId
          ? state.messages.map((m) =>
              m.id === state.streamingMessageId
                ? {
                    ...m,
                    content: state.streamingContent || m.content,
                    ...(msgTokens !== null && msgTokens > 0
                      ? { tokens: msgTokens }
                      : {}),
                  }
                : m,
            )
          : state.messages,
      });

      if (convId) {
        chatService.incrementGeneration(convId).catch(() => {});
      }

      if (convId && !usage) {
        const conv = state.conversations.find((c) => c.id === convId);
        chatService
          .getConversationTokenUsage(
            convId,
            resolveContextWindowTokens(
              conv?.model_id ?? "",
              conv?.provider_id ?? null,
            ),
          )
          .then((summary) => {
            if (get().currentConversationId === convId) {
              set({
                contextUsage: {
                  used_tokens: summary.total_tokens,
                  max_tokens: summary.context_window_tokens,
                  percentage: summary.percentage,
                },
              });
            }
          })
          .catch(() => {});
      }
    },
    errorStream: (error: ChatError) => {
      const state = get();
      if (state.streamingMessageId) {
        const retryHint = formatRetryAfter(error.retry_after_ms);
        const displayMessage = retryHint
          ? `${error.message ?? ""} ${retryHint}`
          : (error.message ?? error.code);
        set({
          messages: state.messages.map((m) =>
            m.id === state.streamingMessageId
              ? { ...m, content: state.streamingContent || m.content, error }
              : m,
          ),
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
          retryStatus: null,
        });
        console.warn(
          `[chat-error] code=${error.code} retryable=${error.retryable} message=${displayMessage}`,
        );
      }
    },
    setRetryStatus: (retry: RetryNotification | null) => {
      set({ retryStatus: retry });
    },
  };

  const envPort: ChatEnvPort = {
    getActiveConversationId: () => get().currentConversationId,
    getIsStreaming: () => get().isStreaming,
    getStreamingMessageId: () => get().streamingMessageId,
    getMessages: () => get().messages,
    getConversations: () => get().conversations,
    getSelectedModeId: () => useAgentModeStore.getState().selectedModeId,
  };

  const orchestrator: ChatOrchestrator = createChatOrchestrator({
    state: statePort,
    env: envPort,
    loadConversations: () => get().loadConversations(),
    switchConversation: (id: string) => get().switchConversation(id),
    renameConversation: (id: string, title: string, titleSource: string) =>
      get().renameConversation(id, title, titleSource),
  });

  _orchestrator = orchestrator;

  return {
    conversations: [],
    currentConversationId: null,
    messages: [],
    isStreaming: false,
    streamingContent: "",
    streamingMessageId: null,
    isReasoningStreaming: false,
    hasMoreMessages: false,
    isLoadingMoreMessages: false,
    isLoadingConversations: false,
    totalMessageCount: 0,
    contextUsage: { used_tokens: 0, max_tokens: 128_000, percentage: 0 },
    isFull: false,
    compactSummary: null,
    compactError: null,
    retryStatus: null,

    loadConversations: async (silent?: boolean) => {
      if (!silent) set({ isLoadingConversations: true });
      try {
        const convs = await chatService.listConversations();
        set({ conversations: convs });
        await get().ensureCurrentConversation();
      } catch (e) {
        console.error("Failed to load conversations:", e);
      } finally {
        if (!silent) set({ isLoadingConversations: false });
      }
    },

    ensureCurrentConversation: async () => {
      const state = get();
      if (state.currentConversationId) return;
      if (state.conversations.length > 0) {
        await get().switchConversation(state.conversations[0].id);
        return;
      }
      try {
        const provList = await chatService.listProviders();
        if (provList.length === 0) return;
        const p = provList[0];
        const id = await chatService.createConversation(
          p.id,
          p.default_model || "",
        );
        await get().loadConversations();
        await get().switchConversation(id);
      } catch (e) {
        console.error("Failed to create initial conversation:", e);
      }
    },

    createConversation: async (providerId: string, modelId: string) => {
      try {
        const id = await chatService.createConversation(providerId, modelId);
        await get().loadConversations();
        await get().switchConversation(id);
        return id;
      } catch (e) {
        console.error("[chatStore] createConversation failed:", e);
        return "";
      }
    },

    switchConversation: async (id: string) => {
      const prevConvId = get().currentConversationId;
      if (get().isStreaming) {
        if (prevConvId) {
          chatService.cancelChat(prevConvId).catch(() => {});
        }
        set({
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
        });
      }
      try {
        const detail = await chatService.getConversationPaginated(
          id,
          MESSAGES_PAGE_SIZE,
          null,
        );
        if (detail) {
          set({
            currentConversationId: id,
            messages: detail.messages || [],
            isStreaming: false,
            streamingContent: "",
            streamingMessageId: null,
            isReasoningStreaming: false,
            hasMoreMessages:
              (detail.messages || []).length < detail.total_count,
            totalMessageCount: detail.total_count,
            contextUsage: {
              used_tokens: 0,
              max_tokens: 128_000,
              percentage: 0,
            },
            isFull: detail.conversation?.is_full ?? false,
            compactSummary: null,
            compactError: null,
          });
          chatService
            .getConversationTokenUsage(
              id,
              resolveContextWindowTokens(
                detail.conversation.model_id,
                detail.conversation.provider_id,
              ),
            )
            .then((summary) => {
              if (get().currentConversationId === id) {
                set({
                  contextUsage: {
                    used_tokens: summary.total_tokens,
                    max_tokens: summary.context_window_tokens,
                    percentage: summary.percentage,
                  },
                });
              }
            })
            .catch(() => {});
        }
      } catch (e) {
        console.error("Failed to switch conversation:", e);
      }
    },

    deleteConversation: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.deleteConversation(id);
      });
      const state = get();
      if (state.currentConversationId === id) {
        set({
          currentConversationId: null,
          messages: [],
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
        });
      }
      await get().loadConversations();
    },

    renameConversation: async (
      id: string,
      title: string,
      titleSource?: string,
    ) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.renameConversation(
            id,
            title,
            titleSource || "user",
          );
        });
        await get().loadConversations();
      } catch (e) {
        console.error("[chatStore] renameConversation failed:", e);
      }
    },

    updateModel: async (id: string, modelId: string) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.updateConversationModel(id, modelId);
        });
        await get().loadConversations();
      } catch (e) {
        console.error("[chatStore] updateModel failed:", e);
      }
    },

    sendMessage: (content: string) => orchestrator.send(content),

    cancelStream: () => orchestrator.cancel(),

    appendDelta: statePort.appendDelta,

    finishStream: statePort.finishStream,

    errorStream: statePort.errorStream,

    deleteMessage: async (id: string) => {
      const state = get();
      set({
        messages: state.messages.filter((m) => m.id !== id),
        totalMessageCount: Math.max(0, state.totalMessageCount - 1),
      });
      try {
        await persistQueue.enqueue(async () => {
          await chatService.deleteMessagePersist(id);
        });
      } catch (e) {
        console.error("[chatStore] deleteMessage failed:", e);
      }
    },

    editMessage: async (messageId: string, newContent: string) => {
      const state = get();
      const msg = state.messages.find((m) => m.id === messageId);
      if (!msg) return;

      if (state.isStreaming && state.currentConversationId) {
        await chatService
          .cancelChat(state.currentConversationId)
          .catch(() => {});
        set({
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
        });
      }

      set({
        messages: state.messages
          .filter((m) => m.timestamp < msg.timestamp || m.id === messageId)
          .map((m) => (m.id === messageId ? { ...m, content: newContent } : m)),
      });

      try {
        await persistQueue.enqueue(async () => {
          await chatService.editMessage(messageId, newContent);
        });
      } catch (e) {
        console.error("[chatStore] editMessage failed:", e);
      }

      if (msg.role === "user") {
        void get().sendMessage(newContent);
      }
    },

    regenerateMessage: async (messageId: string) => {
      const state = get();
      const convId = state.currentConversationId;
      if (!convId) return;

      const msg = state.messages.find((m) => m.id === messageId);
      if (!msg || msg.role !== "assistant") return;

      const lastUserMsg = [...state.messages]
        .reverse()
        .find((m) => m.role === "user" && m.timestamp < msg.timestamp);
      if (!lastUserMsg) return;

      set({
        messages: state.messages.filter(
          (m) => m.timestamp < msg.timestamp && m.id !== messageId,
        ),
      });

      try {
        await persistQueue.enqueue(async () => {
          await chatService.regenerateMessage(convId, messageId);
        });
      } catch (e) {
        console.error("[chatStore] regenerateMessage failed:", e);
      }

      void get().sendMessage(lastUserMsg.content);
    },

    loadMoreMessages: async () => {
      const state = get();
      if (state.isLoadingMoreMessages || !state.hasMoreMessages) return;
      if (state.messages.length === 0) return;

      set({ isLoadingMoreMessages: true });
      try {
        const earliestTimestamp = state.messages[0].timestamp;
        const detail = await chatService.getConversationPaginated(
          state.currentConversationId,
          MESSAGES_PAGE_SIZE,
          earliestTimestamp,
        );
        if (detail && detail.messages.length > 0) {
          const olderMessages = detail.messages.filter(
            (m) => m.timestamp < earliestTimestamp,
          );
          if (olderMessages.length > 0) {
            set((s) => ({
              messages: [...olderMessages, ...s.messages],
              hasMoreMessages: olderMessages.length >= MESSAGES_PAGE_SIZE,
            }));
          } else {
            set({ hasMoreMessages: false });
          }
        } else {
          set({ hasMoreMessages: false });
        }
      } catch (e) {
        console.error("Failed to load more messages:", e);
      } finally {
        set({ isLoadingMoreMessages: false });
      }
    },

    togglePin: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.toggleConversationPin(id);
      });
      await get().loadConversations();
    },

    duplicateConversation: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.duplicateConversation(id);
      });
      await get().loadConversations();
    },

    batchDelete: async (ids: string[]) => {
      await persistQueue.enqueue(async () => {
        await chatService.batchDeleteConversations(ids);
      });
      const state = get();
      if (
        state.currentConversationId &&
        ids.includes(state.currentConversationId)
      ) {
        set({
          currentConversationId: null,
          messages: [],
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
        });
      }
      await get().loadConversations();
    },

    purgeAll: async () => {
      await persistQueue.enqueue(async () => {
        await chatService.purgeConversations();
      });
      set({
        conversations: [],
        currentConversationId: null,
        messages: [],
        isStreaming: false,
        streamingContent: "",
        streamingMessageId: null,
        isReasoningStreaming: false,
      });
    },

    manualCompact: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      const summaryMsgId = `temp-compact-${Date.now()}`;
      const summaryMsg: Message = {
        id: summaryMsgId,
        conversation_id: convId,
        role: "compact_summary",
        content: "",
        tokens: null,
        timestamp: Date.now(),
      };
      set({
        messages: [...get().messages, summaryMsg],
        isStreaming: true,
        streamingContent: "",
        streamingMessageId: summaryMsgId,
      });
      try {
        await chatService.manualCompactConversation(convId);
      } catch (e) {
        console.error("Manual compact failed:", e);
        set({
          messages: get().messages.filter((m) => m.id !== summaryMsgId),
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
        });
      }
    },

    unlockConversation: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      try {
        await chatService.unlockConversation(convId);
      } catch (e) {
        console.error("Unlock conversation failed:", e);
      }
    },

    clearCompactError: () => {
      set({ compactError: null });
    },
  };
});

export { CONTEXT_WARN_THRESHOLD, CONTEXT_DANGER_THRESHOLD };
export {
  createChatOrchestrator,
  type ChatOrchestrator,
  type ChatStatePort,
  type ChatEnvPort,
  type ChatDeltaPayload,
} from "../services/chatOrchestrator";

export function getChatOrchestrator(): ChatOrchestrator | null {
  return _orchestrator;
}
