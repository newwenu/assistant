import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import {
  CHAT_AUTO_TITLE_LENGTH,
  MESSAGES_PAGE_SIZE,
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../config/constants";
import {
  type ChatError,
  ChatErrorCode,
  toChatError,
  formatRetryAfter,
} from "../chat/chatErrors";
import { useAgentModeStore } from "./agentModeStore";
import { useProviderStore, type ModelConfig } from "./providerStore";

export interface Message {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parent_id?: string | null;
  is_pinned?: boolean;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Conversation {
  id: string;
  title: string;
  title_source: string;
  provider_id: string | null;
  model_id: string;
  system_prompt: string | null;
  is_pinned: boolean;
  sort_order: number;
  generation: number;
  created_at: number;
  updated_at: number;
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
  context_window_tokens?: number;
}

export interface ContextUsage {
  used_tokens: number;
  max_tokens: number;
  percentage: number;
}

interface PersistEntry {
  fn: () => Promise<void>;
  resolve: () => void;
  reject: (e: unknown) => void;
}

class PersistQueue {
  private queue: PersistEntry[] = [];
  private running = false;

  enqueue(fn: () => Promise<void>): Promise<void> {
    return new Promise((resolve, reject) => {
      this.queue.push({ fn, resolve, reject });
      void this.runNext();
    });
  }

  private async runNext() {
    if (this.running || this.queue.length === 0) return;
    this.running = true;
    const entry = this.queue.shift()!;
    try {
      await entry.fn();
      entry.resolve();
    } catch (e) {
      entry.reject(e);
    } finally {
      this.running = false;
      void this.runNext();
    }
  }
}

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

function fallbackTitleFromUserMessage(
  userMessage: string,
  maxLength = CHAT_AUTO_TITLE_LENGTH,
): string {
  const firstLine = userMessage.split("\n").find((l) => l.trim()) || "";
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

interface ChatState {
  conversations: Conversation[];
  currentConversationId: string | null;
  messages: Message[];
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  isReasoningStreaming: boolean;
  sidebarOpen: boolean;
  hasMoreMessages: boolean;
  isLoadingMoreMessages: boolean;
  totalMessageCount: number;
  searchResults: Conversation[] | null;
  searchQuery: string;
  searchFullText: boolean;
  filterProviderId: string | null;
  contextUsage: ContextUsage;

  loadConversations: () => Promise<void>;
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
  toggleSidebar: () => void;
  deleteMessage: (id: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  regenerateMessage: (messageId: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  searchConversations: (query: string) => Promise<void>;
  clearSearch: () => void;
  toggleSearchFullText: () => void;
  togglePin: (id: string) => Promise<void>;
  duplicateConversation: (id: string) => Promise<void>;
  exportConversation: (id: string, format: string) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  purgeAll: () => Promise<void>;
  setFilterProviderId: (id: string | null) => void;
}

let sendInProgress = false;

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  currentConversationId: null,
  messages: [],
  isStreaming: false,
  streamingContent: "",
  streamingMessageId: null,
  isReasoningStreaming: false,
  sidebarOpen: false,
  hasMoreMessages: false,
  isLoadingMoreMessages: false,
  totalMessageCount: 0,
  searchResults: null,
  searchQuery: "",
  searchFullText: false,
  filterProviderId: null,
  contextUsage: { used_tokens: 0, max_tokens: 128_000, percentage: 0 },

  loadConversations: async () => {
    try {
      const result = await invoke("list_conversations", { providerId: null });
      const convs = (result as Conversation[]) || [];
      set({ conversations: convs });
      await get().ensureCurrentConversation();
    } catch (e) {
      console.error("Failed to load conversations:", e);
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
      const providers = await invoke("list_providers");
      const provList =
        (providers as { id: string; default_model: string }[]) || [];
      if (provList.length === 0) return;
      const p = provList[0];
      const id = await invoke<string>("create_conversation", {
        providerId: p.id,
        modelId: p.default_model || "",
      });
      await get().loadConversations();
      await get().switchConversation(id);
    } catch (e) {
      console.error("Failed to create initial conversation:", e);
    }
  },

  createConversation: async (providerId: string, modelId: string) => {
    try {
      const id = await invoke<string>("create_conversation", {
        providerId,
        modelId,
      });
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
        invoke("cancel_chat", { conversationId: prevConvId }).catch(() => {});
      }
      set({
        isStreaming: false,
        streamingContent: "",
        streamingMessageId: null,
        isReasoningStreaming: false,
      });
    }
    try {
      const result = await invoke("get_conversation_paginated", {
        id,
        limit: MESSAGES_PAGE_SIZE,
        beforeTimestamp: null,
      });
      const detail = result as {
        conversation: Conversation;
        messages: Message[];
        total_count: number;
      } | null;
      if (detail) {
        set({
          currentConversationId: id,
          messages: detail.messages || [],
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
          hasMoreMessages: (detail.messages || []).length < detail.total_count,
          totalMessageCount: detail.total_count,
          contextUsage: { used_tokens: 0, max_tokens: 128_000, percentage: 0 },
        });
        invoke<{
          total_tokens: number;
          context_window_tokens: number;
          percentage: number;
        }>("get_conversation_token_usage", {
          conversationId: id,
          contextWindowTokens: resolveContextWindowTokens(
            detail.conversation.model_id,
            detail.conversation.provider_id,
          ),
        })
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
      await invoke("delete_conversation", { id });
    });
    const state = get();
    if (state.currentConversationId === id) {
      set({
        currentConversationId: null,
        messages: [],
        isStreaming: false,
        streamingContent: "",
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
        await invoke("rename_conversation", {
          id,
          title,
          titleSource: titleSource || "user",
        });
      });
      await get().loadConversations();
    } catch (e) {
      console.error("[chatStore] renameConversation failed:", e);
    }
  },

  updateModel: async (id: string, modelId: string) => {
    try {
      await persistQueue.enqueue(async () => {
        await invoke("update_conversation_model", { id, modelId });
      });
      await get().loadConversations();
    } catch (e) {
      console.error("[chatStore] updateModel failed:", e);
    }
  },

  sendMessage: async (content: string) => {
    if (sendInProgress) return;
    sendInProgress = true;
    try {
      const state = get();
      if (state.isStreaming && state.currentConversationId) {
        await invoke("cancel_chat", {
          conversationId: state.currentConversationId,
        }).catch(() => {});
        set({
          isStreaming: false,
          streamingContent: "",
          streamingMessageId: null,
          isReasoningStreaming: false,
        });
      }
      if (!state.currentConversationId) {
        try {
          const providers = await invoke("list_providers");
          const provList =
            (providers as { id: string; default_model: string }[]) || [];
          if (provList.length === 0) {
            const errConvId = `err-${Date.now()}`;
            const userMsg: Message = {
              id: `temp-user-${Date.now()}`,
              conversation_id: errConvId,
              role: "user",
              content,
              tokens: null,
              timestamp: Date.now(),
            };
            const assistantMsgId = `temp-assistant-${Date.now()}`;
            set({
              messages: [
                userMsg,
                {
                  id: assistantMsgId,
                  conversation_id: errConvId,
                  role: "assistant",
                  content: "",
                  tokens: null,
                  timestamp: Date.now() + 1,
                },
              ],
              isStreaming: true,
              streamingMessageId: assistantMsgId,
            });
            get().errorStream({
              code: ChatErrorCode.ApiKeyMissing,
              retry_after_ms: null,
              retryable: false,
            });
            return;
          }
          const p = provList[0];
          const modeId = useAgentModeStore.getState().selectedModeId;
          const newId = await invoke<string>("create_conversation", {
            providerId: p.id,
            modelId: p.default_model || "",
            modeId,
          });
          set({ currentConversationId: newId });
          await get().loadConversations();
        } catch (e) {
          get().errorStream({
            code: ChatErrorCode.InternalError,
            message: String(e),
            retry_after_ms: null,
            retryable: false,
          });
          return;
        }
      }

      const convId = get().currentConversationId!;
      const currentState = get();
      const userMsg: Message = {
        id: `temp-user-${Date.now()}`,
        conversation_id: convId,
        role: "user",
        content,
        tokens: null,
        timestamp: Date.now(),
      };
      const assistantMsgId = `temp-assistant-${Date.now()}`;
      const assistantMsg: Message = {
        id: assistantMsgId,
        conversation_id: convId,
        role: "assistant",
        content: "",
        tokens: null,
        timestamp: Date.now() + 1,
      };
      set({
        messages: [...currentState.messages, userMsg, assistantMsg],
        isStreaming: true,
        streamingContent: "",
        streamingMessageId: assistantMsgId,
        contextUsage: { used_tokens: 0, max_tokens: 128_000, percentage: 0 },
      });

      const conv = currentState.conversations.find((c) => c.id === convId);
      if (conv && conv.title_source === "placeholder") {
        const fbTitle = fallbackTitleFromUserMessage(content);
        void get().renameConversation(convId, fbTitle, "fallback");
      }

      try {
        const modeId = useAgentModeStore.getState().selectedModeId;
        await invoke("send_chat_message", {
          conversationId: convId,
          content,
          modeId,
        });
      } catch (e) {
        console.error("Failed to send message:", e);
        get().errorStream(toChatError(e));
      }
    } finally {
      sendInProgress = false;
    }
  },

  cancelStream: async () => {
    const state = get();
    if (!state.currentConversationId) return;
    try {
      await invoke("cancel_chat", {
        conversationId: state.currentConversationId,
      });
    } catch (e) {
      console.error("Failed to cancel chat:", e);
    }
    set({ isStreaming: false });
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
      invoke("increment_generation", { id: convId }).catch(() => {});
    }

    if (convId && !usage) {
      const conv = state.conversations.find((c) => c.id === convId);
      invoke<{
        total_tokens: number;
        context_window_tokens: number;
        percentage: number;
      }>("get_conversation_token_usage", {
        conversationId: convId,
        contextWindowTokens: resolveContextWindowTokens(
          conv?.model_id ?? "",
          conv?.provider_id ?? null,
        ),
      })
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
      });
      console.warn(
        `[chat-error] code=${error.code} retryable=${error.retryable} message=${displayMessage}`,
      );
    }
  },

  toggleSidebar: () => {
    set({ sidebarOpen: !get().sidebarOpen });
  },

  deleteMessage: async (id: string) => {
    const state = get();
    set({
      messages: state.messages.filter((m) => m.id !== id),
      totalMessageCount: Math.max(0, state.totalMessageCount - 1),
    });
    try {
      await persistQueue.enqueue(async () => {
        await invoke("delete_message_persist", { messageId: id });
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
      await invoke("cancel_chat", {
        conversationId: state.currentConversationId,
      }).catch(() => {});
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
        await invoke("edit_message", { messageId, newContent });
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
        await invoke("regenerate_message", {
          conversationId: convId,
          messageId,
        });
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
      const result = await invoke("get_conversation_paginated", {
        id: state.currentConversationId,
        limit: MESSAGES_PAGE_SIZE,
        beforeTimestamp: earliestTimestamp,
      });
      const detail = result as { messages: Message[] } | null;
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

  searchConversations: async (query: string) => {
    set({ searchQuery: query });
    if (!query.trim()) {
      set({ searchResults: null });
      return;
    }
    try {
      if (get().searchFullText) {
        const result = await invoke("search_conversations", { query });
        set({ searchResults: (result as Conversation[]) || [] });
      } else {
        const q = query.toLowerCase();
        const filtered = get().conversations.filter((c) =>
          c.title.toLowerCase().includes(q),
        );
        set({ searchResults: filtered });
      }
    } catch (e) {
      console.error("Failed to search conversations:", e);
      set({ searchResults: [] });
    }
  },

  clearSearch: () => {
    set({ searchResults: null, searchQuery: "" });
  },

  toggleSearchFullText: () => {
    set({ searchFullText: !get().searchFullText });
  },

  togglePin: async (id: string) => {
    await persistQueue.enqueue(async () => {
      await invoke("toggle_conversation_pin", { id });
    });
    await get().loadConversations();
  },

  duplicateConversation: async (id: string) => {
    await persistQueue.enqueue(async () => {
      await invoke("duplicate_conversation", { id });
    });
    await get().loadConversations();
  },

  exportConversation: async (id: string, format: string) => {
    try {
      const content = await invoke<string>("export_conversation", {
        id,
        format,
      });
      const ext = format === "json" ? "json" : "md";
      const mime = format === "json" ? "application/json" : "text/markdown";
      const blob = new Blob([content], { type: mime });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `conversation.${ext}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("Failed to export conversation:", e);
    }
  },

  batchDelete: async (ids: string[]) => {
    await persistQueue.enqueue(async () => {
      await invoke("batch_delete_conversations", { ids });
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
      });
    }
    await get().loadConversations();
  },

  purgeAll: async () => {
    await persistQueue.enqueue(async () => {
      await invoke("purge_conversations");
    });
    set({
      conversations: [],
      currentConversationId: null,
      messages: [],
      isStreaming: false,
      streamingContent: "",
    });
  },

  setFilterProviderId: (id: string | null) => {
    set({ filterProviderId: id });
  },
}));

export { CONTEXT_WARN_THRESHOLD, CONTEXT_DANGER_THRESHOLD };
