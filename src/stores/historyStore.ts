import { create } from "zustand";
import { historyService } from "../services/historyService";
import { chatService } from "../services/chatService";
import { useChatStore } from "./chatStore";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import type { Message } from "../chat/core/chatTypes";
import { createLogger } from "@/services/logger";

const log = createLogger("historyStore");

interface HistoryState {
  selectedId: string | null;
  messages: Message[];
  totalCount: number;
  hasMore: boolean;
  loadingMore: boolean;
  loadingMessages: boolean;
  collapsedGroups: Set<string>;
  multiSelectMode: boolean;
  selectedConversationIds: Set<string>;
  focusedIndex: number;

  selectConversation: (id: string) => Promise<void>;
  clearSelection: () => void;
  refreshMessages: () => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  toggleGroup: (key: string) => void;
  loadConversations: () => Promise<void>;
  enterMultiSelect: () => void;
  exitMultiSelect: () => void;
  toggleConversationSelection: (id: string) => void;
  selectAllConversations: (ids: string[]) => void;
  clearConversationSelections: () => void;
  setFocusedIndex: (index: number) => void;
}

export const useHistoryStore = create<HistoryState>()((set, get) => ({
  selectedId: null,
  messages: [],
  totalCount: 0,
  hasMore: false,
  loadingMore: false,
  loadingMessages: false,
  collapsedGroups: new Set(),
  multiSelectMode: false,
  selectedConversationIds: new Set(),
  focusedIndex: -1,

  selectConversation: async (id: string) => {
    set({ selectedId: id, loadingMessages: true, messages: [] });
    try {
      const detail = await historyService.getConversationAllMessages(id);
      if (detail) {
        set({
          messages: detail.messages || [],
          totalCount: detail.totalCount,
          hasMore: false,
        });
      }
    } catch (e) {
      log.error("Failed to load messages", e);
    } finally {
      set({ loadingMessages: false });
    }
  },

  clearSelection: () => {
    set({ selectedId: null, messages: [], totalCount: 0 });
  },

  refreshMessages: async () => {
    const { selectedId } = get();
    if (!selectedId) return;
    try {
      const detail =
        await historyService.getConversationAllMessages(selectedId);
      if (detail) {
        set({
          messages: detail.messages || [],
          totalCount: detail.totalCount,
          hasMore: false,
        });
      }
    } catch (e) {
      log.error("Failed to refresh messages", e);
    }
  },

  loadMoreMessages: async () => {
    const state = get();
    if (
      state.loadingMore ||
      !state.hasMore ||
      !state.selectedId ||
      state.messages.length === 0
    )
      return;
    set({ loadingMore: true });
    try {
      const earliestTimestamp = state.messages[0].timestamp;
      const detail = await historyService.getConversationPaginated(
        state.selectedId,
        MESSAGES_PAGE_SIZE,
        earliestTimestamp,
      );
      if (detail && detail.messages.length > 0) {
        const older = detail.messages.filter(
          (m) => m.timestamp < earliestTimestamp,
        );
        if (older.length > 0) {
          set((s) => ({
            messages: [...older, ...s.messages],
            hasMore: older.length >= MESSAGES_PAGE_SIZE,
          }));
        } else {
          set({ hasMore: false });
        }
      } else {
        set({ hasMore: false });
      }
    } catch (e) {
      log.error("Failed to load more messages", e);
    } finally {
      set({ loadingMore: false });
    }
  },

  toggleGroup: (key: string) => {
    set((s) => {
      const next = new Set(s.collapsedGroups);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return { collapsedGroups: next };
    });
  },

  loadConversations: async () => {
    try {
      const convs = await chatService.listConversations();
      useChatStore.getState().updateConversations(convs);
    } catch (e) {
      log.error("Failed to load conversations", e);
    }
  },

  enterMultiSelect: () => {
    set({ multiSelectMode: true, selectedConversationIds: new Set() });
  },

  exitMultiSelect: () => {
    set({ multiSelectMode: false, selectedConversationIds: new Set() });
  },

  toggleConversationSelection: (id: string) => {
    set((s) => {
      const next = new Set(s.selectedConversationIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selectedConversationIds: next };
    });
  },

  selectAllConversations: (ids: string[]) => {
    set({ selectedConversationIds: new Set(ids) });
  },

  clearConversationSelections: () => {
    set({ selectedConversationIds: new Set() });
  },

  setFocusedIndex: (index: number) => {
    set({ focusedIndex: index });
  },
}));
