import React, { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { message } from "antd";
import type { Message } from "../chat/core/chatTypes";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import { useChatStore } from "../stores/chatStore";
import { PersistQueue } from "../utils/persistQueue";
import { HistorySidebar } from "./sidebar/HistorySidebar";
import { HistoryDetail } from "./detail/HistoryDetail";
import { useHistorySearch } from "./core/useHistorySearch";
import { useHistoryRefresh } from "./core/useHistoryRefresh";
import css from "./HistoryManager.module.css";

const historyPersistQueue = new PersistQueue();

export const HistoryManager: React.FC = () => {
  const { t } = useTranslation();
  const conversations = useChatStore((s) => s.conversations);
  const isLoadingConversations = useChatStore((s) => s.isLoadingConversations);
  const chatStoreLoadConversations = useChatStore((s) => s.loadConversations);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );

  const {
    searchQuery,
    searchResults,
    searchFullText,
    setSearchFullText,
    handleSearch,
  } = useHistorySearch(conversations);

  const handleMessagesRefreshed = useCallback(
    (refreshedMessages: Message[], refreshedTotal: number) => {
      setMessages(refreshedMessages);
      setTotalCount(refreshedTotal);
      setHasMore(refreshedMessages.length < refreshedTotal);
    },
    [],
  );

  const { refreshing, handleRefresh } = useHistoryRefresh(
    chatStoreLoadConversations,
    selectedId,
    handleMessagesRefreshed,
  );

  useEffect(() => {
    void chatStoreLoadConversations();
  }, [chatStoreLoadConversations]);

  const selectConversation = useCallback(async (id: string) => {
    setSelectedId(id);
    setLoadingMessages(true);
    setMessages([]);
    try {
      const result = await invoke("get_conversation_paginated", {
        id,
        limit: MESSAGES_PAGE_SIZE,
        beforeTimestamp: null,
      });
      const detail = result as {
        messages: Message[];
        total_count: number;
      } | null;
      if (detail) {
        setMessages(detail.messages || []);
        setTotalCount(detail.total_count);
        setHasMore((detail.messages || []).length < detail.total_count);
      }
    } catch (e) {
      console.error("Failed to load messages:", e);
    } finally {
      setLoadingMessages(false);
    }
  }, []);

  const loadMoreMessages = useCallback(async () => {
    if (loadingMore || !hasMore || !selectedId || messages.length === 0) return;
    setLoadingMore(true);
    try {
      const earliestTimestamp = messages[0].timestamp;
      const result = await invoke("get_conversation_paginated", {
        id: selectedId,
        limit: MESSAGES_PAGE_SIZE,
        beforeTimestamp: earliestTimestamp,
      });
      const detail = result as { messages: Message[] } | null;
      if (detail && detail.messages.length > 0) {
        const older = detail.messages.filter(
          (m) => m.timestamp < earliestTimestamp,
        );
        if (older.length > 0) {
          setMessages((prev) => [...older, ...prev]);
          setHasMore(older.length >= MESSAGES_PAGE_SIZE);
        } else {
          setHasMore(false);
        }
      } else {
        setHasMore(false);
      }
    } catch (e) {
      console.error("Failed to load more messages:", e);
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, hasMore, selectedId, messages]);

  const deleteMessage = useCallback(
    async (msgId: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await invoke("delete_message_persist", { messageId: msgId });
        });
        setMessages((prev) => prev.filter((m) => m.id !== msgId));
        setTotalCount((prev) => Math.max(0, prev - 1));
        await chatStoreLoadConversations(true);
        message.success(t("chat.message.message_deleted"));
      } catch (e) {
        console.error("Failed to delete message:", e);
        message.error(t("chat.message.delete_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const deleteConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await invoke("delete_conversation", { id });
        });
        if (selectedId === id) {
          setSelectedId(null);
          setMessages([]);
          setTotalCount(0);
        }
        await chatStoreLoadConversations(true);
        message.success(t("chat.history.conversation_deleted"));
      } catch (e) {
        console.error("Failed to delete conversation:", e);
        message.error(t("chat.history.operation_failed"));
      }
    },
    [selectedId, chatStoreLoadConversations, t],
  );

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await invoke("rename_conversation", { id, title });
        });
        await chatStoreLoadConversations(true);
        message.success(t("chat.history.renamed"));
      } catch (e) {
        console.error("Failed to rename:", e);
        message.error(t("chat.history.rename_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const exportConversation = useCallback(
    async (id: string, format: string = "markdown") => {
      try {
        const content = await invoke<string>("export_conversation", {
          id,
          format,
        });
        const ext = format === "json" ? "json" : "md";
        const mime =
          format === "json"
            ? "application/json;charset=utf-8"
            : "text/markdown;charset=utf-8";
        const blob = new Blob([content], { type: mime });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `conversation-${id}.${ext}`;
        a.click();
        URL.revokeObjectURL(url);
        message.success(t("chat.history.export_success"));
      } catch (e) {
        console.error("Export failed:", e);
        message.error(t("chat.history.export_failed"));
      }
    },
    [t],
  );

  const togglePin = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await invoke("toggle_conversation_pin", { id });
        });
        await chatStoreLoadConversations(true);
      } catch (e) {
        console.error("Failed to toggle pin:", e);
        message.error(t("chat.history.operation_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const duplicateConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await invoke("duplicate_conversation", { id });
        });
        await chatStoreLoadConversations(true);
        message.success(t("chat.history.duplicated"));
      } catch (e) {
        console.error("Failed to duplicate:", e);
        message.error(t("chat.history.duplicate_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const toggleGroup = useCallback((key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const selectedConv = conversations.find((c) => c.id === selectedId);

  return (
    <div className={css.root}>
      <HistorySidebar
        conversations={conversations}
        isLoadingConversations={isLoadingConversations}
        selectedId={selectedId}
        searchQuery={searchQuery}
        searchResults={searchResults}
        searchFullText={searchFullText}
        refreshing={refreshing}
        collapsedGroups={collapsedGroups}
        onSearch={handleSearch}
        onToggleFullText={() => setSearchFullText((prev) => !prev)}
        onRefresh={() => void handleRefresh()}
        onSelect={(id) => void selectConversation(id)}
        onDelete={(id) => void deleteConversation(id)}
        onRename={(id, title) => void renameConversation(id, title)}
        onExport={(id, format) => void exportConversation(id, format)}
        onTogglePin={(id) => void togglePin(id)}
        onDuplicate={(id) => void duplicateConversation(id)}
        onToggleGroup={toggleGroup}
      />
      <HistoryDetail
        selectedId={selectedId}
        selectedConv={selectedConv}
        messages={messages}
        totalCount={totalCount}
        hasMore={hasMore}
        loadingMore={loadingMore}
        loadingMessages={loadingMessages}
        onExport={(id, format) => void exportConversation(id, format)}
        onDeleteConversation={(id) => void deleteConversation(id)}
        onDeleteMessage={(id) => void deleteMessage(id)}
        onLoadMore={() => void loadMoreMessages()}
      />
    </div>
  );
};
