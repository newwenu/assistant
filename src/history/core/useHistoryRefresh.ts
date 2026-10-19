import { useState, useRef, useCallback, useEffect } from "react";
import { chatService } from "../../services/chatService";
import { useChatStore } from "../../stores/chatStore";
import { createLogger } from "@/services/logger";

const log = createLogger("useHistoryRefresh");

const REFRESH_INTERVAL_MS = 60000;

interface UseHistoryRefreshReturn {
  refreshing: boolean;
  refreshConversations: () => Promise<void>;
  handleRefresh: () => Promise<void>;
}

export function useHistoryRefresh(
  refreshMessages: () => Promise<void>,
): UseHistoryRefreshReturn {
  const [refreshing, setRefreshing] = useState(false);
  const refreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshConversations = useCallback(async () => {
    setRefreshing(true);
    try {
      const convs = await chatService.listConversations();
      useChatStore.getState().updateConversations(convs);
    } catch (e) {
      log.error("Failed to refresh conversations", e);
    } finally {
      setRefreshing(false);
    }
  }, []);

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
    handleRefresh,
  };
}
