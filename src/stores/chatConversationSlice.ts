import type { Conversation, ContextUsage } from "../chat/core/chatTypes";
import type { ChatError } from "../chat/core/chatErrors";
import type { Message, TruncationStrategy } from "../chat/core/chatTypes";
import { chatService } from "../services/chatService";
import { providerService } from "../services/providerService";
import { createLogger } from "@/services/logger";
import { notifyError } from "./notifyStore";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import type { PersistQueue } from "../utils/persistQueue";
import { useChatStreamStore } from "./chatStreamStore";
import type { ChatOrchestrator } from "../services/chatOrchestrator";
import {
  fetchTokenUsage,
  type FetchTokenUsageDeps,
} from "../chat/core/chatUtils";

const log = createLogger("chatConversationSlice");

type FullState = ChatConversationSlice & {
  messages: Message[];
  contextUsage: ContextUsage;
  totalMessageCount: number;
  hasMoreMessages: boolean;
  isFull: boolean;
  compactSummary: string | null;
  compactError: ChatError | null;
  resendFrom: (
    pivotTimestamp: number,
    contentToSend: string,
    strategy?: TruncationStrategy,
  ) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
};

export interface ChatConversationSlice {
  conversations: Conversation[];
  currentConversationId: string | null;
  isLoadingConversations: boolean;
  activeBranchId: string | null;
  branchOperationSeq: number;

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
  togglePin: (id: string) => Promise<void>;
  duplicateConversation: (id: string) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  purgeAll: () => Promise<void>;
  switchBranch: (messageId: string) => Promise<void>;
  updateConversations: (convs: Conversation[]) => void;
}

export function createConversationSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
  fetchTokenUsageDeps: FetchTokenUsageDeps,
  persistQueue: PersistQueue,
  getOrchestrator: () => ChatOrchestrator | null,
): ChatConversationSlice {
  return {
    conversations: [],
    currentConversationId: null,
    isLoadingConversations: false,
    activeBranchId: null,
    branchOperationSeq: 0,

    loadConversations: async (silent?: boolean) => {
      if (!silent) set({ isLoadingConversations: true });
      try {
        const convs = await chatService.listConversations();
        set({ conversations: convs });
        await get().ensureCurrentConversation();
      } catch (e) {
        log.error("Failed to load conversations", e);
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
        const provList = await providerService.listProviders();
        if (provList.length === 0) return;
        const p = provList[0];
        const id = await chatService.createConversation(
          p.id,
          p.defaultModel || "",
        );
        await get().loadConversations();
        await get().switchConversation(id);
      } catch (e) {
        log.error("Failed to create initial conversation", e);
      }
    },

    createConversation: async (providerId: string, modelId: string) => {
      try {
        const id = await chatService.createConversation(providerId, modelId);
        await get().loadConversations();
        await get().switchConversation(id);
        return id;
      } catch (e) {
        log.error("createConversation failed", e);
        return "";
      }
    },

    switchConversation: async (id: string) => {
      if (useChatStreamStore.getState().isStreaming) {
        const orchestrator = getOrchestrator();
        if (orchestrator) {
          await orchestrator.ensureIdle();
        } else {
          const prevConvId = get().currentConversationId;
          if (prevConvId) {
            await chatService
              .cancelChat(prevConvId)
              .catch((e) =>
                log.warn("cancelChat failed in switchConversation", e),
              );
          }
          useChatStreamStore.getState().resetStreaming();
        }
      }
      try {
        const detail = await chatService.getConversationPaginated(
          id,
          MESSAGES_PAGE_SIZE,
          null,
        );
        if (detail) {
          const hasBranch = !!detail.conversation?.activeBranchId;
          set({
            currentConversationId: id,
            messages: detail.messages || [],
            activeBranchId: detail.conversation?.activeBranchId ?? null,
            hasMoreMessages:
              !hasBranch && (detail.messages || []).length < detail.totalCount,
            totalMessageCount: detail.totalCount,
            contextUsage: {
              usedTokens: 0,
              maxTokens: 128_000,
              percentage: 0,
            },
            isFull: detail.conversation?.isFull ?? false,
            compactSummary: null,
            compactError: null,
          });
          fetchTokenUsage(
            id,
            detail.conversation.modelId,
            detail.conversation.providerId,
            fetchTokenUsageDeps,
          );
        }
      } catch (e) {
        log.error("Failed to switch conversation", e);
      }
    },

    deleteConversation: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.deleteConversation(id);
      });
      const state = get();
      if (state.currentConversationId === id) {
        useChatStreamStore.getState().resetStreaming();
        set({
          currentConversationId: null,
          messages: [],
        });
      }
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
      } catch (e) {
        log.error("renameConversation failed", e);
        notifyError(
          "chat.errors.renameFailed.title",
          "chat.errors.renameFailed.description",
        );
      }
    },

    updateModel: async (id: string, modelId: string) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.updateConversationModel(id, modelId);
        });
      } catch (e) {
        log.error("updateModel failed", e);
        notifyError(
          "chat.errors.updateModelFailed.title",
          "chat.errors.updateModelFailed.description",
        );
      }
    },

    togglePin: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.toggleConversationPin(id);
      });
    },

    duplicateConversation: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.duplicateConversation(id);
      });
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
        useChatStreamStore.getState().resetStreaming();
        set({
          currentConversationId: null,
          messages: [],
        });
      }
    },

    purgeAll: async () => {
      await persistQueue.enqueue(async () => {
        await chatService.purgeConversations();
      });
      useChatStreamStore.getState().resetStreaming();
      set({
        conversations: [],
        currentConversationId: null,
        messages: [],
      });
    },

    switchBranch: async (messageId: string) => {
      const state = get();
      const convId = state.currentConversationId;
      if (!convId) return;

      set((s) => ({ branchOperationSeq: s.branchOperationSeq + 1 }));
      const seq = get().branchOperationSeq;

      try {
        await chatService.switchBranch(convId, messageId, seq);
      } catch (e) {
        log.error("switchBranch failed", e);
        notifyError(
          "chat.errors.switchBranchFailed.title",
          "chat.errors.switchBranchFailed.description",
        );
      }
    },

    updateConversations: (convs: Conversation[]) => {
      set({ conversations: convs });
    },
  };
}
