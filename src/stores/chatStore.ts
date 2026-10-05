import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  CHAT_AUTO_TITLE_LENGTH,
  MESSAGES_PAGE_SIZE,
} from "../config/constants";
import {
  createChatDeltaCoalescer,
  type CoalescibleDelta,
} from "../chat/streamCoalescer";
import {
  type ChatError,
  ChatErrorCode,
  toChatError,
  formatRetryAfter,
} from "../chat/chatErrors";

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
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Conversation {
  id: string;
  title: string;
  provider_id: string | null;
  model_id: string;
  system_prompt: string | null;
  created_at: number;
  updated_at: number;
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
}

interface ChatState {
  conversations: Conversation[];
  currentConversationId: string | null;
  messages: Message[];
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  sidebarOpen: boolean;
  hasMoreMessages: boolean;
  isLoadingMoreMessages: boolean;
  totalMessageCount: number;
  searchResults: Conversation[] | null;
  searchQuery: string;

  loadConversations: () => Promise<void>;
  createConversation: (providerId: string, modelId: string) => Promise<string>;
  ensureCurrentConversation: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  renameConversation: (id: string, title: string) => Promise<void>;
  updateModel: (id: string, modelId: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelStream: () => Promise<void>;
  appendDelta: (delta: string) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  toggleSidebar: () => void;
  deleteMessage: (id: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  searchConversations: (query: string) => Promise<void>;
  clearSearch: () => void;
}

let chatDeltaUnlisten: (() => void) | null = null;

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  currentConversationId: null,
  messages: [],
  isStreaming: false,
  streamingContent: "",
  streamingMessageId: null,
  sidebarOpen: false,
  hasMoreMessages: false,
  isLoadingMoreMessages: false,
  totalMessageCount: 0,
  searchResults: null,
  searchQuery: "",

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
    const id = await invoke<string>("create_conversation", {
      providerId,
      modelId,
    });
    await get().loadConversations();
    await get().switchConversation(id);
    return id;
  },

  switchConversation: async (id: string) => {
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
          hasMoreMessages: (detail.messages || []).length < detail.total_count,
          totalMessageCount: detail.total_count,
        });
      }
    } catch (e) {
      console.error("Failed to switch conversation:", e);
    }
  },

  deleteConversation: async (id: string) => {
    try {
      await invoke("delete_conversation", { id });
    } catch (e) {
      console.error("Failed to delete conversation:", e);
    }
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

  renameConversation: async (id: string, title: string) => {
    try {
      await invoke("rename_conversation", { id, title });
    } catch (e) {
      console.error("Failed to rename conversation:", e);
    }
    await get().loadConversations();
  },

  updateModel: async (id: string, modelId: string) => {
    try {
      await invoke("update_conversation_model", { id, modelId });
    } catch (e) {
      console.error("Failed to update model:", e);
    }
    await get().loadConversations();
  },

  sendMessage: async (content: string) => {
    const state = get();
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
            message: "请先配置 AI Provider（在设置中添加）",
            retry_after_ms: null,
            retryable: false,
          });
          return;
        }
        const p = provList[0];
        const newId = await invoke<string>("create_conversation", {
          providerId: p.id,
          modelId: p.default_model || "",
        });
        set({ currentConversationId: newId });
        await get().loadConversations();
      } catch (e) {
        get().errorStream(toChatError(`创建对话失败: ${e}`));
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
    });

    try {
      await invoke("send_chat_message", {
        conversationId: convId,
        content,
      });
    } catch (e) {
      console.error("Failed to send message:", e);
      get().errorStream(toChatError(e));
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
    set({ streamingContent: newContent });
    if (state.streamingMessageId) {
      set({
        messages: state.messages.map((m) =>
          m.id === state.streamingMessageId ? { ...m, content: newContent } : m,
        ),
      });
    }
  },

  finishStream: (_usage?: TokenUsage) => {
    const state = get();
    set({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
    });
    if (state.currentConversationId && state.messages.length > 0) {
      const lastAssistant = [...state.messages]
        .reverse()
        .find((m) => m.role === "assistant");
      if (
        lastAssistant &&
        lastAssistant.content.length > 0 &&
        state.conversations.find((c) => c.id === state.currentConversationId)
          ?.title === "新对话"
      ) {
        const autoTitle = lastAssistant.content.slice(
          0,
          CHAT_AUTO_TITLE_LENGTH,
        );
        get().renameConversation(state.currentConversationId, autoTitle);
      }
    }
  },

  errorStream: (error: ChatError) => {
    const state = get();
    if (state.streamingMessageId) {
      const retryHint = formatRetryAfter(error.retry_after_ms);
      const displayMessage = retryHint
        ? `${error.message} ${retryHint}`
        : error.message;
      set({
        messages: state.messages.map((m) =>
          m.id === state.streamingMessageId
            ? { ...m, content: m.content, error }
            : m,
        ),
        isStreaming: false,
        streamingContent: "",
        streamingMessageId: null,
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
      await invoke("delete_message_persist", { messageId: id });
    } catch (e) {
      console.error("Failed to delete message:", e);
    }
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
      const result = await invoke("search_conversations", { query });
      set({ searchResults: (result as Conversation[]) || [] });
    } catch (e) {
      console.error("Failed to search conversations:", e);
      set({ searchResults: [] });
    }
  },

  clearSearch: () => {
    set({ searchResults: null, searchQuery: "" });
  },
}));

export async function initChatDeltaListener() {
  if (chatDeltaUnlisten) return;
  const store = useChatStore;

  const coalescer = createChatDeltaCoalescer<CoalescibleDelta>((event) => {
    if (event.delta) {
      store.getState().appendDelta(event.delta);
    }
  });

  chatDeltaUnlisten = await listen("chat-delta", (event) => {
    const payload = event.payload as {
      conversation_id: string;
      message_id: string;
      delta_text?: string;
      delta_reasoning?: string;
      delta_tool_call_id?: string;
      delta_tool_call_name?: string;
      delta_tool_call_arguments?: string;
      finish_reason?: string;
      usage?: TokenUsage;
      error?: unknown;
    };
    if (payload.error) {
      coalescer.flush();
      store.getState().errorStream(toChatError(payload.error));
      return;
    }
    if (payload.delta_text) {
      coalescer.push({
        conversation_id: payload.conversation_id,
        message_id: payload.message_id,
        delta: payload.delta_text,
      });
    }
    if (payload.delta_reasoning) {
      const state = store.getState();
      if (state.streamingMessageId) {
        store.setState({
          messages: state.messages.map((m) =>
            m.id === state.streamingMessageId
              ? {
                  ...m,
                  reasoning: (m.reasoning || "") + payload.delta_reasoning,
                }
              : m,
          ),
        });
      }
    }
    if (
      payload.delta_tool_call_id ||
      payload.delta_tool_call_name ||
      payload.delta_tool_call_arguments
    ) {
      const state = store.getState();
      if (state.streamingMessageId) {
        store.setState({
          messages: state.messages.map((m) => {
            if (m.id !== state.streamingMessageId) return m;
            const toolCalls = [...(m.toolCalls || [])];
            if (payload.delta_tool_call_id) {
              toolCalls.push({
                id: payload.delta_tool_call_id,
                name: payload.delta_tool_call_name || "",
                arguments: payload.delta_tool_call_arguments || "",
              });
            } else if (toolCalls.length > 0) {
              const last = toolCalls[toolCalls.length - 1];
              if (payload.delta_tool_call_name)
                last.name += payload.delta_tool_call_name;
              if (payload.delta_tool_call_arguments)
                last.arguments += payload.delta_tool_call_arguments;
            }
            return { ...m, toolCalls };
          }),
        });
      }
    }
    if (payload.finish_reason) {
      coalescer.flush();
      store.getState().finishStream(payload.usage);
    }
  });
}
