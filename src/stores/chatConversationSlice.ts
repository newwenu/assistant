import type {
  Conversation,
  ContextUsage,
  ConversationToolConfig,
} from "../chat/core/chatTypes";
import type { Message, TruncationStrategy } from "../chat/core/chatTypes";
import { chatService } from "../services/chatService";
import { providerService } from "../services/providerService";
import { createLogger } from "@/services/logger";
import { notifyError } from "./notifyStore";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import type { PersistQueue } from "../utils/persistQueue";
import { useChatStreamStore } from "./chatStreamStore";
import { useAgentModeStore } from "./agentModeStore";
import type { ChatOrchestrator } from "../services/chatOrchestrator";
import {
  fetchTokenUsage,
  type FetchTokenUsageDeps,
} from "../chat/core/chatUtils";
import { DEFAULT_REASONING_CHOICE } from "../chat/core/reasoning";
import type { ReasoningChoice } from "../chat/core/reasoning";

const log = createLogger("chatConversationSlice");

type FullState = ChatConversationSlice & {
  messages: Message[];
  contextUsage: ContextUsage;
  totalMessageCount: number;
  hasMoreMessages: boolean;
  isFull: boolean;
  resendFrom: (
    pivotTimestamp: number,
    contentToSend: string,
    strategy?: TruncationStrategy,
  ) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  reasoningChoice: ReasoningChoice;
};

export interface ChatConversationSlice {
  conversations: Conversation[];
  currentConversationId: string | null;
  isLoadingConversations: boolean;
  activeBranchId: string | null;
  branchOperationSeq: number;

  loadConversations: (silent?: boolean) => Promise<void>;
  createConversation: (
    providerId: string,
    modelId: string,
    modeId?: string | null,
  ) => Promise<string>;
  ensureCurrentConversation: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  renameConversation: (
    id: string,
    title: string,
    titleSource?: string,
  ) => Promise<void>;
  updateModel: (id: string, modelId: string) => Promise<void>;
  setConversationMode: (id: string, modeId: string) => Promise<boolean>;
  setConversationToolConfig: (
    id: string,
    toolConfig: ConversationToolConfig,
  ) => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  duplicateConversation: (id: string) => Promise<void>;
  batchDelete: (ids: string[]) => Promise<void>;
  purgeAll: () => Promise<void>;
  switchBranch: (messageId: string) => Promise<void>;
  refreshTokenUsage: () => void;
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
          "builtin-default",
        );
        await get().loadConversations();
        await get().switchConversation(id);
      } catch (e) {
        log.error("Failed to create initial conversation", e);
      }
    },

    createConversation: async (
      providerId,
      modelId,
      modeId = "builtin-default",
    ) => {
      try {
        const id = await chatService.createConversation(
          providerId,
          modelId,
          modeId,
        );
        await get().loadConversations();
        await get().switchConversation(id);
        return id;
      } catch (e) {
        log.error("createConversation failed", e);
        return "";
      }
    },

    switchConversation: async (id: string) => {
      /**
       * 切会话**不取消**后台 turn（C5.2 / A4）：后台轮次由后端继续推进，
       * 这里只把本地流式视图换成目标会话的状态；目标会话仍在运行时由
       * `session-state-changed` 驱动恢复（`onConversationSwitch`）。
       */
      const orchestrator = getOrchestrator();
      if (orchestrator) {
        orchestrator.onConversationSwitch(id);
      } else {
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
              hasUsage: false,
            },
            isFull: detail.conversation?.isFull ?? false,
            reasoningChoice: DEFAULT_REASONING_CHOICE,
          });
          useAgentModeStore
            .getState()
            .selectMode(detail.conversation?.modeId ?? "builtin-default");
          fetchTokenUsage(id, fetchTokenUsageDeps);
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
        const usage = get().contextUsage;
        set({
          contextUsage: {
            usedTokens: usage.usedTokens,
            hasUsage: usage.hasUsage,
          },
        });
      } catch (e) {
        log.error("updateModel failed", e);
        notifyError(
          "chat.errors.updateModelFailed.title",
          "chat.errors.updateModelFailed.description",
        );
      }
    },

    setConversationMode: async (id: string, modeId: string) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.patchConversation(id, { modeId });
        });
        set((state) => ({
          conversations: state.conversations.map((c) =>
            c.id === id ? { ...c, modeId } : c,
          ),
        }));
        return true;
      } catch (e) {
        log.error("setConversationMode failed", e);
        notifyError(
          "chat.errors.updateModeFailed.title",
          "chat.errors.updateModeFailed.description",
          undefined,
          typeof e === "string" && e.trim() ? e : undefined,
        );
        return false;
      }
    },

    setConversationToolConfig: async (id, toolConfig) => {
      const previous = get().conversations.find((c) => c.id === id)?.toolConfig;
      set((state) => ({
        conversations: state.conversations.map((c) =>
          c.id === id ? { ...c, toolConfig } : c,
        ),
      }));
      try {
        await persistQueue.enqueue(async () => {
          await chatService.patchConversation(id, { toolConfig });
        });
      } catch (e) {
        log.error("setConversationToolConfig failed", e);
        set((state) => ({
          conversations: state.conversations.map((c) =>
            c.id === id ? { ...c, toolConfig: previous ?? null } : c,
          ),
        }));
        notifyError(
          "chat.errors.updateToolConfigFailed.title",
          "chat.errors.updateToolConfigFailed.description",
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

    refreshTokenUsage: () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      fetchTokenUsage(convId, fetchTokenUsageDeps);
    },

    updateConversations: (convs: Conversation[]) => {
      set({ conversations: convs });
    },
  };
}
