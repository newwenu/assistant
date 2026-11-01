import { tauriInvoke as invoke } from "./tauriInvoke";
import type {
  Conversation,
  ConversationDetail,
  Message,
  SiblingInfo,
  TokenUsageSummary,
} from "../chat/core/chatTypes";

export type { ConversationDetail, TokenUsageSummary };

export const chatService = {
  listConversations: (): Promise<Conversation[]> =>
    invoke<Conversation[]>("list_conversations", { providerId: null }).then(
      (r) => r || [],
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
    invoke<ConversationDetail | null>("get_conversation", {
      id,
      opts: { mode: "paginated", limit, beforeTimestamp },
    }).then((r) => r ?? null),

  getConversationAllMessages: (
    id: string,
  ): Promise<ConversationDetail | null> =>
    invoke<ConversationDetail | null>("get_conversation", {
      id,
      opts: { mode: "all" },
    }).then((r) => r ?? null),

  getConversationTokenUsage: (
    conversationId: string,
    contextWindowTokens: number | undefined,
  ): Promise<TokenUsageSummary | null> =>
    invoke<TokenUsageSummary | null>("get_conversation_token_usage", {
      conversationId,
      contextWindowTokens,
    }).then((r) => r ?? null),

  deleteConversation: (id: string): Promise<void> =>
    invoke("delete_conversation", { id }),

  patchConversation: (
    id: string,
    patch: {
      title?: string;
      titleSource?: string;
      modelId?: string;
      providerId?: string;
    },
  ): Promise<void> => invoke("patch_conversation", { id, patch }),

  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ): Promise<void> => chatService.patchConversation(id, { title, titleSource }),

  updateConversationModel: (id: string, modelId: string): Promise<void> =>
    chatService.patchConversation(id, { modelId }),

  updateConversationProviderAndModel: (
    id: string,
    providerId: string,
    modelId: string,
  ): Promise<void> =>
    chatService.patchConversation(id, { providerId, modelId }),

  sendChatMessage: (
    conversationId: string,
    content: string,
    modeId: string | null | undefined,
    branchPointMessageId: string | null | undefined,
    providerId: string,
    modelId: string,
    reasoningLevel: string | null = null,
  ): Promise<void> =>
    invoke("send_chat_message", {
      conversationId,
      content,
      modeId,
      branchPointMessageId,
      providerId,
      modelId,
      reasoningLevel,
    }),

  cancelChat: (conversationId: string): Promise<void> =>
    invoke("cancel_chat", { conversationId }),

  incrementGeneration: (id: string): Promise<void> =>
    invoke("increment_generation", { id }),

  deleteMessagePersist: (messageId: string): Promise<void> =>
    invoke("delete_message_persist", { messageId }),

  getMessageSubtreeSize: (messageId: string): Promise<number> =>
    invoke<number>("get_message_subtree_size", { messageId }),

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

  createBranch: (
    conversationId: string,
    branchPointMessageId: string,
    newRole: string,
    newContent: string,
    _seq?: number,
  ): Promise<Message> =>
    invoke<Message>("create_branch", {
      conversationId,
      branchPointMessageId,
      newRole,
      newContent,
      _seq,
    }),

  switchBranch: (
    conversationId: string,
    targetMessageId: string,
    _seq?: number,
  ): Promise<void> =>
    invoke("switch_branch", { conversationId, targetMessageId, _seq }),

  getMessageSiblings: (messageId: string): Promise<SiblingInfo[]> =>
    invoke<SiblingInfo[]>("get_message_siblings", { messageId }),

  forkToNewConversation: (
    sourceConversationId: string,
    branchLeafMessageId: string,
  ): Promise<string> =>
    invoke<string>("fork_to_new_conversation", {
      sourceConversationId,
      branchLeafMessageId,
    }),
};
