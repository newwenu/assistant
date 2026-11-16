import type {
  Conversation,
  ContextUsage,
  ConversationToolConfig,
} from "../../chat/core/chatTypes";
import type {
  AgentToolCall,
  Message,
  TruncationStrategy,
} from "../../chat/core/chatTypes";
import {
  applyPendingToolCalls,
  findToolCallOwner,
} from "../../chat/core/toolCallView";
import { chatService } from "../../services/chat/chatService";
import { providerService } from "../../services/providerService";
import { createLogger } from "@/services/system/logger";
import { notifyError } from "../system/notifyStore";
import { MESSAGES_PAGE_SIZE } from "../../config/constants";
import type { PersistQueue } from "../../utils/persistQueue";
import { useChatStreamStore } from "./chatStreamStore";
import { useAgentModeStore } from "../agent/agentModeStore";
import type { TurnOrchestrator } from "../../services/stream/turnOrchestrator";
import { resolveEffectiveToolConfig } from "../../chat/core/toolConfig";
import type { ProviderModelRef } from "../../chat/core/providerModel";
import {
  fetchTokenUsage,
  type FetchTokenUsageDeps,
} from "../../chat/core/chatUtils";
import { DEFAULT_REASONING_CHOICE } from "../../chat/core/reasoning";
import type { ReasoningChoice } from "../../chat/core/reasoning";

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
  syncSelectionFromConversation: (ref?: ProviderModelRef) => void;
};

export interface ChatConversationSlice {
  conversations: Conversation[];
  currentConversationId: string | null;
  /** 当前会话的工作区根；空串 = 未配置（I38h §8.1，永不出现 `null`）。 */
  workspaceRoot: string;
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
  /** 写入会话工作区绑定；空串 = 清除。成功后同步本地 `workspaceRoot`。 */
  setConversationWorkspace: (
    id: string,
    workspaceRoot: string,
  ) => Promise<boolean>;
  setConversationToolConfig: (
    id: string,
    toolConfig: ConversationToolConfig,
  ) => Promise<void>;
  /** 会话级工具配置缺省时按模式初始值播种并落库，幂等（I38d §4.4）。 */
  ensureConversationToolConfig: (id: string) => Promise<void>;
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
  getOrchestrator: () => TurnOrchestrator | null,
): ChatConversationSlice {
  /** 同一会话的工具配置播种只做一次：写入失败回滚为 `null` 后不再自动重试，避免死循环（I38d §4.4）。 */
  const seededConversationIds = new Set<string>();

  return {
    conversations: [],
    currentConversationId: null,
    workspaceRoot: "",
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
      /**
       * 挂起提问的补拉与历史加载并行：`ask_user` 的等待期在库里只有「已发起、
       * 无结果行」，历史视图会渲染成 `missing`，不补拉就看不到可作答的卡片。
       * 补拉失败不能拖垮整个切会话，退化为「无挂起提问」。
       */
      const pendingPromise = chatService.listPendingQuestions(id).catch((e) => {
        log.error("Failed to list pending questions", e);
        return [] as AgentToolCall[];
      });
      try {
        const detail = await chatService.getConversationPaginated(
          id,
          MESSAGES_PAGE_SIZE,
          null,
        );
        const pending = await pendingPromise;
        if (detail) {
          const baseMessages = detail.messages || [];
          const messages = applyPendingToolCalls(
            baseMessages,
            pending,
            Date.now(),
          );
          const hasBranch = !!detail.conversation?.activeBranchId;
          set({
            currentConversationId: id,
            messages,
            activeBranchId: detail.conversation?.activeBranchId ?? null,
            workspaceRoot: detail.conversation?.workspaceRoot ?? "",
            hasMoreMessages:
              !hasBranch && baseMessages.length < detail.totalCount,
            totalMessageCount: detail.totalCount,
            contextUsage: {
              usedTokens: 0,
              hasUsage: false,
            },
            isFull: detail.conversation?.isFull ?? false,
            reasoningChoice: DEFAULT_REASONING_CHOICE,
          });
          /**
           * 快照同时是「轮次仍在进行」的唯一证据：窗口重载后内存里的
           * phase / turnId 为空，必须靠它恢复流式视图，否则作答后的事件会被丢弃。
           */
          if (orchestrator && pending.length > 0) {
            const call = pending[0];
            const owner = findToolCallOwner(baseMessages, call.callId);
            if (!owner) {
              log.warn(
                `pending question ${call.callId} has no loaded owner message`,
              );
            }
            orchestrator.onPendingToolCalls(
              id,
              call.turnId,
              owner?.id ?? null,
              call.iteration,
            );
          }
          useAgentModeStore
            .getState()
            .selectMode(detail.conversation?.modeId ?? "builtin-default");
          /** 选中项跟随会话，属 store 不变式，不再由视图挂 effect 回写。 */
          get().syncSelectionFromConversation({
            providerId: detail.conversation?.providerId ?? null,
            modelId: detail.conversation?.modelId ?? "",
          });
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
          workspaceRoot: "",
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

    setConversationWorkspace: async (id: string, workspaceRoot: string) => {
      try {
        await persistQueue.enqueue(async () => {
          await chatService.patchConversation(
            id,
            workspaceRoot === ""
              ? { clearWorkspaceRoot: true }
              : { workspaceRoot },
          );
        });
        if (get().currentConversationId === id) set({ workspaceRoot });
        set((state) => ({
          conversations: state.conversations.map((c) =>
            c.id === id ? { ...c, workspaceRoot } : c,
          ),
        }));
        return true;
      } catch (e) {
        log.error("setConversationWorkspace failed", e);
        notifyError(
          "chat.errors.updateWorkspaceFailed.title",
          "chat.errors.updateWorkspaceFailed.description",
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

    ensureConversationToolConfig: async (id) => {
      if (seededConversationIds.has(id)) return;
      const conv = get().conversations.find((c) => c.id === id);
      if (!conv || conv.toolConfig) return;
      const { modes } = useAgentModeStore.getState();
      /** 模式尚未加载完：不播种也不标记，等模式到位后重新调用。 */
      if (modes.length === 0) return;
      const mode = modes.find((m) => m.id === conv.modeId);
      seededConversationIds.add(id);
      await get().setConversationToolConfig(
        id,
        resolveEffectiveToolConfig(null, mode?.toolConfig),
      );
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
          workspaceRoot: "",
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
        workspaceRoot: "",
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
      get().syncSelectionFromConversation();
    },
  };
}
