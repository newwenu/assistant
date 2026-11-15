import { tauriInvoke as invoke } from "./tauriInvoke";
import { chatService } from "./chatService";
import type { Conversation } from "../chat/core/chatTypes";
import type {
  ConversationDetailDto,
  MessageRecordDto,
} from "../types/protocol.contract";

export const historyService = {
  getConversationPaginated: chatService.getConversationPaginated,
  getConversationAllMessages: chatService.getConversationAllMessages,
  deleteConversation: chatService.deleteConversation,
  renameConversation: chatService.renameConversation,
  toggleConversationPin: chatService.toggleConversationPin,
  duplicateConversation: chatService.duplicateConversation,
  getMessageSubtreeSize: chatService.getMessageSubtreeSize,
  deleteMessagePersist: chatService.deleteMessagePersist,

  getMessageRecords: (conversationId: string): Promise<MessageRecordDto[]> =>
    invoke<ConversationDetailDto | null>("get_conversation", {
      id: conversationId,
      opts: { mode: "all", includeRecords: true },
    }).then((r) => r?.records ?? []),

  searchConversations: (query: string): Promise<Conversation[]> =>
    invoke<Conversation[]>("search_conversations", { query }).then(
      (r) => r || [],
    ),

  exportConversationToFile: (
    id: string,
    format: string,
    path: string,
  ): Promise<void> => invoke("export_conversation", { id, format, path }),
};
