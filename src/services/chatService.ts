import { tauriInvoke as invoke } from "./tauriInvoke";
import type {
  ConversationDetailDto,
  ToolDescriptorDto,
} from "../types/protocol.contract";
import { toChatMessage } from "../chat/core/toolCallView";
import type {
  AgentQuestionAnswer,
  AgentToolCall,
  Conversation,
  ConversationDetail,
  ConversationToolConfig,
  Message,
  SendChatMessageArgs,
  SiblingInfo,
  TokenUsageSummary,
  ToolApprovalDecision,
} from "../chat/core/chatTypes";

export type { ConversationDetail, TokenUsageSummary };

/**
 * 压缩运行期取消的判定（I41 §1）：后端取消分支返回该文案，属「用户停止」而非失败，
 * 前端不提示、不当作错误呈现。文案比较集中在此处，禁止散落到调用方。
 */
export function isCompactCancelled(error: unknown): boolean {
  return typeof error === "string" && error.includes("Compaction cancelled");
}

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
    beforeTurnIndex: number | null,
  ): Promise<ConversationDetail | null> =>
    invoke<ConversationDetailDto | null>("get_conversation", {
      id,
      opts: { mode: "paginated", limit, beforeTurnIndex },
    }).then((r) =>
      r ? { ...r, messages: r.messages.map(toChatMessage) } : null,
    ),

  getConversationAllMessages: (
    id: string,
  ): Promise<ConversationDetail | null> =>
    invoke<ConversationDetailDto | null>("get_conversation", {
      id,
      opts: { mode: "all" },
    }).then((r) =>
      r ? { ...r, messages: r.messages.map(toChatMessage) } : null,
    ),

  getConversationTokenUsage: (
    conversationId: string,
    contextWindowTokens?: number,
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
      modeId?: string;
      toolConfig?: ConversationToolConfig;
      workspaceRoot?: string;
      clearWorkspaceRoot?: boolean;
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
    args: SendChatMessageArgs,
  ): Promise<void> => invoke("send_chat_message", { conversationId, args }),

  sendAgentMessage: (
    conversationId: string,
    args: SendChatMessageArgs,
  ): Promise<void> => invoke("send_agent_message", { conversationId, args }),

  cancelChat: (conversationId: string): Promise<void> =>
    invoke("cancel_chat", { conversationId }),

  cancelAgent: (conversationId: string): Promise<void> =>
    invoke("cancel_agent", { conversationId }),

  /** 审批回填：`approve` / `deny` / `allowSession` / `allowAlways`（I38d §3.3）。 */
  resolveAgentToolApproval: (args: ToolApprovalDecision): Promise<void> =>
    invoke("resolve_agent_tool_call", { args }),

  /** 提问回填：`ask_user` 卡片的作答结果（I38c §9）。 */
  resolveAgentQuestion: (args: AgentQuestionAnswer): Promise<void> =>
    invoke("resolve_agent_question", { args }),

  /**
   * 挂起提问的补拉：切回会话 / 窗口重载时重建 `ask_user` 卡片。
   * 快照里的 `timeoutMs` 已被后端改写为剩余毫秒（I38c §9）。
   */
  listPendingQuestions: (conversationId: string): Promise<AgentToolCall[]> =>
    invoke<AgentToolCall[]>("list_pending_questions", { conversationId }).then(
      (r) => r || [],
    ),

  /** 工具装配清单：全部 agent 受众工具，按 tier 分组供面板渲染（I38d §4.7）。 */
  listAvailableTools: (): Promise<ToolDescriptorDto[]> =>
    invoke<ToolDescriptorDto[]>("list_available_tools").then((r) => r || []),

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
    seq?: number,
  ): Promise<Message> =>
    invoke<Message>("create_branch", {
      conversationId,
      branchPointMessageId,
      newRole,
      newContent,
      seq,
    }),

  switchBranch: (
    conversationId: string,
    targetMessageId: string,
    seq?: number,
  ): Promise<void> =>
    invoke("switch_branch", { conversationId, targetMessageId, seq }),

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
