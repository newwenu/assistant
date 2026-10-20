import type { Message, TruncationStrategy } from "../chat/core/chatTypes";
import { chatService } from "../services/chatService";
import { createLogger } from "@/services/logger";
import { notifyError } from "./errorNotifyStore";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import type { PersistQueue } from "../utils/persistQueue";
import type { ChatOrchestrator } from "../services/chatOrchestrator";

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
  orchestrator: ChatOrchestrator,
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
  };
}
