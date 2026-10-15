import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Message } from "../chat/chatTypes";

export interface ConversationDetail {
  conversation: Conversation;
  messages: Message[];
  total_count: number;
}

export interface TokenUsageSummary {
  total_tokens: number;
  context_window_tokens: number;
  percentage: number;
}

export interface ProviderBrief {
  id: string;
  default_model: string;
}

export const chatService = {
  listConversations: (): Promise<Conversation[]> =>
    invoke("list_conversations", { providerId: null }).then(
      (r) => (r as Conversation[]) || [],
    ),

  listProviders: (): Promise<ProviderBrief[]> =>
    invoke("list_providers").then((r) => (r as ProviderBrief[]) || []),

  createConversation: (
    providerId: string,
    modelId: string,
    modeId?: string | null,
  ): Promise<string> =>
    invoke<string>("create_conversation", { providerId, modelId, modeId }),

  getConversationPaginated: (
    id: string | null,
    limit: number,
    beforeTimestamp: number | null,
  ): Promise<ConversationDetail | null> =>
    invoke("get_conversation_paginated", { id, limit, beforeTimestamp }).then(
      (r) => r as ConversationDetail | null,
    ),

  getConversationTokenUsage: (
    conversationId: string,
    contextWindowTokens: number | undefined,
  ): Promise<TokenUsageSummary> =>
    invoke<TokenUsageSummary>("get_conversation_token_usage", {
      conversationId,
      contextWindowTokens,
    }),

  deleteConversation: (id: string): Promise<void> =>
    invoke("delete_conversation", { id }),

  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ): Promise<void> => invoke("rename_conversation", { id, title, titleSource }),

  updateConversationModel: (id: string, modelId: string): Promise<void> =>
    invoke("update_conversation_model", { id, modelId }),

  sendChatMessage: (
    conversationId: string,
    content: string,
    modeId?: string | null,
  ): Promise<void> =>
    invoke("send_chat_message", { conversationId, content, modeId }),

  cancelChat: (conversationId: string): Promise<void> =>
    invoke("cancel_chat", { conversationId }),

  incrementGeneration: (id: string): Promise<void> =>
    invoke("increment_generation", { id }),

  deleteMessagePersist: (messageId: string): Promise<void> =>
    invoke("delete_message_persist", { messageId }),

  editMessage: (messageId: string, newContent: string): Promise<void> =>
    invoke("edit_message", { messageId, newContent }),

  regenerateMessage: (
    conversationId: string,
    messageId: string,
  ): Promise<void> =>
    invoke("regenerate_message", { conversationId, messageId }),

  toggleConversationPin: (id: string): Promise<void> =>
    invoke("toggle_conversation_pin", { id }),

  duplicateConversation: (id: string): Promise<void> =>
    invoke("duplicate_conversation", { id }),

  batchDeleteConversations: (ids: string[]): Promise<void> =>
    invoke("batch_delete_conversations", { ids }),

  purgeConversations: (): Promise<void> => invoke("purge_conversations"),

  manualCompactConversation: (conversationId: string): Promise<void> =>
    invoke("manual_compact_conversation", { conversationId }),

  unlockConversation: (conversationId: string): Promise<void> =>
    invoke("unlock_conversation", { conversationId }),
};
