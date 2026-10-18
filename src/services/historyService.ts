import { invoke } from "@tauri-apps/api/core";
import { chatService } from "./chatService";
import type { Conversation } from "../chat/core/chatTypes";

export const historyService = {
  getConversationPaginated: chatService.getConversationPaginated,
  getConversationAllMessages: chatService.getConversationAllMessages,

  searchConversations: (query: string): Promise<Conversation[]> =>
    invoke<Conversation[]>("search_conversations", { query }).then(
      (r) => r || [],
    ),

  getMessageSubtreeSize: (messageId: string): Promise<number> =>
    invoke<number>("get_message_subtree_size", { messageId }),

  deleteMessagePersist: (messageId: string): Promise<void> =>
    invoke("delete_message_persist", { messageId }),

  deleteConversation: (id: string): Promise<void> =>
    invoke("delete_conversation", { id }),

  renameConversation: (
    id: string,
    title: string,
    titleSource?: string,
  ): Promise<void> =>
    invoke("rename_conversation", { id, title, titleSource }),

  exportConversation: (id: string, format: string): Promise<string> =>
    invoke<string>("export_conversation", { id, format }),

  toggleConversationPin: (id: string): Promise<void> =>
    invoke("toggle_conversation_pin", { id }),

  duplicateConversation: (id: string): Promise<void> =>
    invoke("duplicate_conversation", { id }),
};