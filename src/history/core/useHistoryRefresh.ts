import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Message } from "../../chat/chatTypes";
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
      const result = await invoke("get_conversation_paginated", {
        id: selectedId,
        limit: MESSAGES_PAGE_SIZE,
        beforeTimestamp: null,
      });
      const detail = result as {
        messages: Message[];
        total_count: number;
      } | null;
      if (detail) {
        onMessagesRefreshed(detail.messages || [], detail.total_count);
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
