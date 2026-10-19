import { create } from "zustand";
import { chatService } from "../services/chatService";
import { providerService } from "../services/providerService";
import { createLogger } from "@/services/logger";
import { notifyError } from "@/services/errorNotifier";
import { type ChatOrchestrator } from "../services/chatOrchestrator";
import {
  MESSAGES_PAGE_SIZE,
  CONTEXT_WARN_THRESHOLD,
  CONTEXT_DANGER_THRESHOLD,
} from "../config/constants";
import type {
  Message,
  Conversation,
  ContextUsage,
  TruncationStrategy,
} from "../chat/core/chatTypes";
import type { ChatError } from "../chat/core/chatErrors";
export type {
  Message,
  ToolCall,
  Conversation,
  TokenUsage,
  ContextUsage,
  TruncationStrategy,
} from "../chat/core/chatTypes";
import { PersistQueue } from "../utils/persistQueue";
import { useChatStreamStore } from "./chatStreamStore";
import {
  createStatePort,
  createEnvPort,
  createOrchestrator,
  fetchTokenUsage,
  type FetchTokenUsageDeps,
} from "./chatStorePorts";

const persistQueue = new PersistQueue();
const log = createLogger("chatStore");

interface ChatState {
  conversations: Conversation[];
  currentConversationId: string | null;
  messages: Message[];
  hasMoreMessages: boolean;
  isLoadingMoreMessages: boolean;
  isLoadingConversations: boolean;
  totalMessageCount: number;
  contextUsage: ContextUsage;
  isFull: boolean;
  compactSummary: string | null;
  compactError: ChatError | null;
  pendingBranchPoint: string | null;
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
  sendMessage: (content: string) => Promise<void>;
  resendFrom: (
    pivotTimestamp: number,
    contentToSend: string,
    strategy?: TruncationStrategy,
  ) => Promise<void>;
  cancelStream: () => Promise<void>;
  deleteMessage: (id: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  regenerateMessage: (messageId: string) => Promise<void>;
  retryFromError: (messageId: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  duplicateConversation: (id: string) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  purgeAll: () => Promise<void>;
  manualCompact: () => Promise<void>;
  unlockConversation: () => Promise<void>;
  clearCompactError: () => void;
  switchBranch: (messageId: string) => Promise<void>;
}

let _orchestrator: ChatOrchestrator | null = null;

export const useChatStore = create<ChatState>()((set, get): ChatState => {
  const fetchTokenUsageDeps: FetchTokenUsageDeps = {
    isActiveConv: (convId: string) => get().currentConversationId === convId,
    setContextUsage: (usage: ContextUsage) => set({ contextUsage: usage }),
  };
  const statePort = createStatePort(set, get, fetchTokenUsageDeps);
  const envPort = createEnvPort(get, set);
  const orchestrator = createOrchestrator(statePort, envPort, {
    loadConversations: () => get().loadConversations(),
    switchConversation: (id: string) => get().switchConversation(id),
    renameConversation: (id: string, title: string, titleSource: string) =>
      get().renameConversation(id, title, titleSource),
  });

  _orchestrator = orchestrator;

  return {
    conversations: [],
    currentConversationId: null,
    messages: [],
    hasMoreMessages: false,
    isLoadingMoreMessages: false,
    isLoadingConversations: false,
    totalMessageCount: 0,
    contextUsage: { usedTokens: 0, maxTokens: 128_000, percentage: 0 },
    isFull: false,
    compactSummary: null,
    compactError: null,
    pendingBranchPoint: null,
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
      const prevConvId = get().currentConversationId;
      if (useChatStreamStore.getState().isStreaming) {
        if (prevConvId) {
          chatService
            .cancelChat(prevConvId)
            .catch((e) =>
              log.warn("cancelChat failed in switchConversation", e),
            );
        }
        useChatStreamStore.getState().resetStreaming();
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
      await get().loadConversations();
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
        await get().loadConversations();
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
        await get().loadConversations();
      } catch (e) {
        log.error("updateModel failed", e);
        notifyError(
          "chat.errors.updateModelFailed.title",
          "chat.errors.updateModelFailed.description",
        );
      }
    },

    sendMessage: (content: string) => orchestrator.send(content),

    resendFrom: async (
      pivotTimestamp: number,
      contentToSend: string,
      strategy?: TruncationStrategy,
    ) => {
      await orchestrator.ensureIdle();

      if (!strategy || strategy.kind === "truncate") {
        const snapshot = get().messages;
        set((state) => ({
          messages: state.messages.filter((m) => m.timestamp < pivotTimestamp),
        }));

        try {
          void get().sendMessage(contentToSend);
        } catch (e) {
          log.error("resendFrom send failed, rolling back", e);
          set({ messages: snapshot });
        }
      } else {
        set({ pendingBranchPoint: strategy.branchPointMessageId });
        try {
          await get().sendMessage(contentToSend);
        } catch (e) {
          log.error("resendFrom branch send failed", e);
          set({ pendingBranchPoint: null });
        }
      }
    },

    cancelStream: () => orchestrator.cancel(),

    deleteMessage: async (id: string) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.deleteMessagePersist(id);
        });
      } catch (e) {
        log.error("deleteMessage failed", e);
        notifyError(
          "chat.errors.deleteMessageFailed.title",
          "chat.errors.deleteMessageFailed.description",
        );
      }
    },

    editMessage: async (messageId: string, newContent: string) => {
      const state = get();
      const msg = state.messages.find((m) => m.id === messageId);
      if (!msg) return;

      if (msg.role === "user") {
        try {
          await get().resendFrom(msg.timestamp, newContent, {
            kind: "branch",
            branchPointMessageId: messageId,
          });
        } catch (e) {
          log.error("editMessage failed", e);
        }
      }
    },

    regenerateMessage: async (messageId: string) => {
      const state = get();
      const convId = state.currentConversationId;
      if (!convId) return;

      const msg = state.messages.find((m) => m.id === messageId);
      if (!msg || msg.role !== "assistant") return;

      const parentMsg = msg.parentId
        ? state.messages.find((m) => m.id === msg.parentId)
        : undefined;
      if (!parentMsg || parentMsg.role !== "user") return;

      try {
        await get().resendFrom(msg.timestamp, parentMsg.content, {
          kind: "branch",
          branchPointMessageId: messageId,
        });
      } catch (e) {
        log.error("regenerateMessage failed", e);
      }
    },

    retryFromError: async (messageId: string) => {
      const state = get();
      const msgIndex = state.messages.findIndex((m) => m.id === messageId);
      if (msgIndex === -1 || state.messages[msgIndex].role !== "assistant")
        return;

      const msg = state.messages[msgIndex];
      const userMsg = [...state.messages]
        .slice(0, msgIndex)
        .reverse()
        .find((m) => m.role === "user");
      if (!userMsg) return;

      try {
        await get().resendFrom(msg.timestamp, userMsg.content, {
          kind: "branch",
          branchPointMessageId: messageId,
        });
      } catch (e) {
        log.error("retryFromError failed", e);
      }
    },

    loadMoreMessages: async () => {
      const state = get();
      if (state.isLoadingMoreMessages || !state.hasMoreMessages) return;
      if (state.messages.length === 0) return;

      set({ isLoadingMoreMessages: true });
      try {
        const earliestTimestamp = state.messages[0].timestamp;
        const detail = await chatService.getConversationPaginated(
          state.currentConversationId,
          MESSAGES_PAGE_SIZE,
          earliestTimestamp,
        );
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
        log.error("Failed to load more messages", e);
      } finally {
        set({ isLoadingMoreMessages: false });
      }
    },

    togglePin: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.toggleConversationPin(id);
      });
      await get().loadConversations();
    },

    duplicateConversation: async (id: string) => {
      await persistQueue.enqueue(async () => {
        await chatService.duplicateConversation(id);
      });
      await get().loadConversations();
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
      await get().loadConversations();
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

    manualCompact: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      const summaryMsgId = `temp-compact-${Date.now()}`;
      const summaryMsg: Message = {
        id: summaryMsgId,
        conversationId: convId,
        role: "compact_summary",
        content: "",
        tokens: null,
        timestamp: Date.now(),
        siblingIndex: 0,
      };
      useChatStreamStore.getState().beginStream(summaryMsgId);
      set({
        messages: [...get().messages, summaryMsg],
      });
      try {
        await chatService.manualCompactConversation(convId);
      } catch (e) {
        log.error("Manual compact failed", e);
        notifyError(
          "chat.errors.compactFailed.title",
          "chat.errors.compactFailed.description",
        );
        useChatStreamStore.getState().cancelStreaming();
        set({
          messages: get().messages.filter((m) => m.id !== summaryMsgId),
        });
      }
    },

    unlockConversation: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      try {
        await chatService.unlockConversation(convId);
      } catch (e) {
        log.error("Unlock conversation failed", e);
      }
    },

    clearCompactError: () => {
      set({ compactError: null });
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
  };
});

export { CONTEXT_WARN_THRESHOLD, CONTEXT_DANGER_THRESHOLD };
export {
  createChatOrchestrator,
  type ChatOrchestrator,
  type ChatStatePort,
  type ChatEnvPort,
} from "../services/chatOrchestrator";
export function getChatOrchestrator(): ChatOrchestrator | null {
  return _orchestrator;
}
