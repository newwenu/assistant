import type { Message, TruncationStrategy } from "../chat/core/chatTypes";
import { chatService } from "../services/chatService";
import { createLogger } from "@/services/logger";
import { notifyError } from "./notifyStore";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import type { PersistQueue } from "../utils/persistQueue";
import type { TurnOrchestrator } from "../services/stream/turnOrchestrator";
import { rebuildContent, parseContent } from "../chat/core/contentBlockUtils";
import { MessageTree } from "../chat/core/messageTree";

const log = createLogger("chatMessageSlice");

type FullState = ChatMessageSlice & { currentConversationId: string | null };

export interface ChatMessageSlice {
  messages: Message[];
  hasMoreMessages: boolean;
  isLoadingMoreMessages: boolean;
  totalMessageCount: number;
  pendingBranchPoint: string | null;

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
}

export function createMessageSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
  orchestrator: TurnOrchestrator,
  persistQueue: PersistQueue,
): ChatMessageSlice {
  return {
    messages: [],
    hasMoreMessages: false,
    isLoadingMoreMessages: false,
    totalMessageCount: 0,
    pendingBranchPoint: null,

    sendMessage: (content: string) => orchestrator.send(content),

    resendFrom: async (
      pivotTimestamp: number,
      contentToSend: string,
      strategy?: TruncationStrategy,
    ) => {
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
        const isStructured = parseContent(newContent) !== null;
        const contentToSend = isStructured
          ? newContent
          : rebuildContent(msg.content, newContent);
        try {
          await get().resendFrom(msg.timestamp, contentToSend, {
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

      const msgIndex = state.messages.findIndex((m) => m.id === messageId);
      if (msgIndex === -1 || state.messages[msgIndex].role !== "assistant")
        return;

      const userContent =
        [...state.messages]
          .slice(0, msgIndex)
          .reverse()
          .find((m) => m.role === "user")?.content ?? "";

      const tree = MessageTree.fromActiveBranch(state.messages);
      const branchPointId = tree.getTurnRootId(messageId);
      const branchPoint = state.messages.find((m) => m.id === branchPointId);

      try {
        await get().resendFrom(
          branchPoint?.timestamp ?? state.messages[msgIndex].timestamp,
          userContent,
          {
            kind: "branch",
            branchPointMessageId: branchPointId,
          },
        );
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

      const tree = MessageTree.fromActiveBranch(state.messages);
      const branchPointId = tree.getTurnRootId(msg.id);
      const branchPoint = state.messages.find((m) => m.id === branchPointId);

      try {
        await get().resendFrom(
          branchPoint?.timestamp ?? msg.timestamp,
          userMsg.content,
          {
            kind: "branch",
            branchPointMessageId: branchPointId,
          },
        );
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
        const oldestTurnIndex =
          state.messages.find((m) => m.turnIndex !== null)?.turnIndex ?? null;
        const detail = await chatService.getConversationPaginated(
          state.currentConversationId,
          MESSAGES_PAGE_SIZE,
          oldestTurnIndex,
        );
        if (detail && detail.messages.length > 0) {
          const known = new Set(state.messages.map((m) => m.id));
          const olderMessages = detail.messages.filter((m) => !known.has(m.id));
          if (olderMessages.length > 0) {
            set((s) => {
              const messages = [...olderMessages, ...s.messages];
              return {
                messages,
                hasMoreMessages: messages.length < detail.totalCount,
              };
            });
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
  };
}
