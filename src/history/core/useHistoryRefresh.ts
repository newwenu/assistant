import { useState, useRef, useCallback, useEffect } from "react";
import { historyService } from "../../services/historyService";
import type { Message } from "../../chat/core/chatTypes";
import { MESSAGES_PAGE_SIZE } from "../../config/constants";

const REFRESH_INTERVAL_MS = 15000;

interface UseHistoryRefreshReturn {
  refreshing: boolean;
  refreshConversations: () => Promise<void>;
  refreshMessages: () => Promise<void>;
  handleRefresh: () => Promise<void>;
}

export function useHistoryRefresh(
  chatStoreLoadConversations: (silent?: boolean) => Promise<void>,
  selectedId: string | null,
  onMessagesRefreshed: (messages: Message[], totalCount: number) => void,
): UseHistoryRefreshReturn {
  const [refreshing, setRefreshing] = useState(false);
  const refreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshConversations = useCallback(async () => {
    setRefreshing(true);
    try {
      await chatStoreLoadConversations(true);
    } catch (e) {
      console.error("Failed to refresh conversations:", e);
    } finally {
      setRefreshing(false);
    }
  }, [chatStoreLoadConversations]);

  const refreshMessages = useCallback(async () => {
    if (!selectedId) return;
    try {
      const detail = await historyService.getConversationPaginated(
        selectedId,
        MESSAGES_PAGE_SIZE,
        null,
      );
      if (detail) {
        onMessagesRefreshed(detail.messages || [], detail.totalCount);
      }
    } catch (e) {
      console.error("Failed to refresh messages:", e);
    }
  }, [selectedId, onMessagesRefreshed]);

  const handleRefresh = useCallback(async () => {
    await Promise.all([refreshConversations(), refreshMessages()]);
  }, [refreshConversations, refreshMessages]);

  useEffect(() => {
    refreshTimerRef.current = setInterval(() => {
      void refreshConversations();
      void refreshMessages();
    }, REFRESH_INTERVAL_MS);
    return () => {
      if (refreshTimerRef.current) clearInterval(refreshTimerRef.current);
    };
  }, [refreshConversations, refreshMessages]);

  return {
    refreshing,
    refreshConversations,
    refreshMessages,
    handleRefresh,
  };
}
