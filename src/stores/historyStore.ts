import { create } from "zustand";
import { historyService } from "../services/historyService";
import { chatService } from "../services/chat/chatService";
import { useChatStore } from "./chat/chatStore";
import type { Message } from "../chat/core/chatTypes";
import { createLogger } from "@/services/system/logger";
import { translateKey } from "@/i18n";

const log = createLogger("historyStore");

interface HistoryState {
  selectedId: string | null;
  messages: Message[];
  totalCount: number;
  loadingMessages: boolean;
  messagesError: string | null;
  collapsedGroups: Set<string>;
  multiSelectMode: boolean;
  selectedConversationIds: Set<string>;
  focusedIndex: number;
  sidebarCollapsed: boolean;

  selectConversation: (id: string) => Promise<void>;
  clearSelection: () => void;
  refreshMessages: () => Promise<void>;
  toggleGroup: (key: string) => void;
  loadConversations: () => Promise<void>;
  enterMultiSelect: () => void;
  exitMultiSelect: () => void;
  toggleConversationSelection: (id: string) => void;
  selectAllConversations: (ids: string[]) => void;
  clearConversationSelections: () => void;
  setFocusedIndex: (index: number) => void;
  toggleSidebarCollapsed: () => void;
}

export const useHistoryStore = create<HistoryState>()((set, get) => ({
  selectedId: null,
  messages: [],
  totalCount: 0,
  loadingMessages: false,
  messagesError: null,
  collapsedGroups: new Set(),
  multiSelectMode: false,
  selectedConversationIds: new Set(),
  focusedIndex: -1,
  sidebarCollapsed: false,

  selectConversation: async (id: string) => {
    set({
      selectedId: id,
      loadingMessages: true,
      messages: [],
      messagesError: null,
    });
    try {
      const detail = await historyService.getConversationAllMessages(id);
      if (detail) {
        set({
          messages: detail.messages || [],
          totalCount: detail.totalCount,
        });
      }
    } catch (e) {
      log.error("Failed to load messages", e);
      set({ messagesError: translateKey("history.load_messages_failed") });
    } finally {
      set({ loadingMessages: false });
    }
  },

  clearSelection: () => {
    set({
      selectedId: null,
      messages: [],
      totalCount: 0,
      messagesError: null,
    });
  },

  refreshMessages: async () => {
    const { selectedId } = get();
    if (!selectedId) return;
    set({ messagesError: null });
    try {
      const detail =
        await historyService.getConversationAllMessages(selectedId);
      if (detail) {
        set({
          messages: detail.messages || [],
          totalCount: detail.totalCount,
        });
      }
    } catch (e) {
      log.error("Failed to refresh messages", e);
      set({ messagesError: translateKey("history.load_messages_failed") });
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
      useChatStore
        .getState()
        .setConversationsError(
          translateKey("history.load_conversations_failed"),
        );
    }
  },

  enterMultiSelect: () => {
    set({
      multiSelectMode: true,
      selectedConversationIds: new Set(),
      sidebarCollapsed: false,
    });
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

  toggleSidebarCollapsed: () => {
    set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed }));
  },
}));
