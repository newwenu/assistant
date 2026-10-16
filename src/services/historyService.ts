import { invoke } from "@tauri-apps/api/core";
import { chatService } from "./chatService";
import type { ConversationRow } from "../chat/core/protocol";
import { rowToConversation } from "../chat/core/protocolAdapter";
import type { Conversation } from "../chat/core/chatTypes";

export const historyService = {
  getConversationPaginated: chatService.getConversationPaginated,

  searchConversations: (query: string): Promise<Conversation[]> =>
    invoke<ConversationRow[]>("search_conversations", { query }).then((r) =>
      (r || []).map(rowToConversation),
    ),

  deleteMessagePersist: (messageId: string): Promise<void> =>
    invoke("delete_message_persist", { messageId }),

  deleteConversation: (id: string): Promise<void> =>
    invoke("delete_conversation", { id }),

  renameConversation: (id: string, title: string): Promise<void> =>
    invoke("rename_conversation", { id, title }),

  exportConversation: (id: string, format: string): Promise<string> =>
    invoke<string>("export_conversation", { id, format }),

  toggleConversationPin: (id: string): Promise<void> =>
    invoke("toggle_conversation_pin", { id }),

  duplicateConversation: (id: string): Promise<void> =>
    invoke("duplicate_conversation", { id }),
};
