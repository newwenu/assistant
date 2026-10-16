import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Message } from "../chat/core/chatTypes";
import {
  invokeList,
  invokeSingle,
  rowToConversation,
  rowToConversationDetail,
  rowToTokenUsageSummary,
} from "../chat/core/protocolAdapter";

export interface ConversationDetail {
  conversation: Conversation;
  messages: Message[];
  totalCount: number;
}

export interface TokenUsageSummary {
  totalTokens: number;
  contextWindowTokens: number;
  percentage: number;
}

export const chatService = {
  listConversations: (): Promise<Conversation[]> =>
    invokeList(
      invoke("list_conversations", { providerId: null }),
      rowToConversation,
    ),

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
    invokeSingle(
      invoke("get_conversation_paginated", { id, limit, beforeTimestamp }),
      rowToConversationDetail,
    ),

  getConversationTokenUsage: (
    conversationId: string,
    contextWindowTokens: number | undefined,
  ): Promise<TokenUsageSummary | null> =>
    invokeSingle(
      invoke("get_conversation_token_usage", {
        conversationId,
        contextWindowTokens,
      }),
      rowToTokenUsageSummary,
    ),

  deleteConversation: (id: string): Promise<void> =>
    invoke("delete_conversation", { id }),

  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ): Promise<void> => invoke("rename_conversation", { id, title, titleSource }),

  updateConversationModel: (id: string, modelId: string): Promise<void> =>
    invoke("update_conversation_model", { id, modelId }),

  updateConversationProvider: (id: string, providerId: string): Promise<void> =>
    invoke("update_conversation_provider", { id, providerId }),

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