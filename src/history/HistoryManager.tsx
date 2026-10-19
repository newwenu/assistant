import React, { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { message, Modal } from "antd";
import type { Message } from "../chat/core/chatTypes";
import { MESSAGES_PAGE_SIZE } from "../config/constants";
import { historyService } from "../services/historyService";
import { useChatStore } from "../stores/chatStore";
import { PersistQueue } from "../utils/persistQueue";
import { createLogger } from "@/services/logger";
import { notifyError } from "@/services/errorNotifier";
import { HistorySidebar } from "./sidebar/HistorySidebar";
import { HistoryDetail } from "./detail/HistoryDetail";
import { useHistorySearch } from "./core/useHistorySearch";
import { useHistoryRefresh } from "./core/useHistoryRefresh";
import css from "./HistoryManager.module.css";

const historyPersistQueue = new PersistQueue();
const log = createLogger("HistoryManager");

export const HistoryManager: React.FC = () => {
  const { t } = useTranslation();
  const [modal, modalContextHolder] = Modal.useModal();
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
      const detail = await historyService.getConversationAllMessages(id);
      if (detail) {
        setMessages(detail.messages || []);
        setTotalCount(detail.totalCount);
        setHasMore(false);
      }
    } catch (e) {
      log.error("Failed to load messages", e);
    } finally {
      setLoadingMessages(false);
    }
  }, []);

  const loadMoreMessages = useCallback(async () => {
    if (loadingMore || !hasMore || !selectedId || messages.length === 0) return;
    setLoadingMore(true);
    try {
      const earliestTimestamp = messages[0].timestamp;
      const detail = await historyService.getConversationPaginated(
        selectedId,
        MESSAGES_PAGE_SIZE,
        earliestTimestamp,
      );
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
      log.error("Failed to load more messages", e);
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, hasMore, selectedId, messages]);

  const reloadMessages = useCallback(async () => {
    if (!selectedId) return;
    try {
      const detail =
        await historyService.getConversationAllMessages(selectedId);
      if (detail) {
        setMessages(detail.messages || []);
        setTotalCount(detail.totalCount);
        setHasMore(false);
      }
    } catch (e) {
      log.error("Failed to reload messages", e);
    }
  }, [selectedId]);

  const deleteMessage = useCallback(
    async (msgId: string) => {
      try {
        const subtreeSize = await historyService.getMessageSubtreeSize(msgId);
        const confirmed = await new Promise<boolean>((resolve) => {
          modal.confirm({
            title: t("chat.message.confirm_delete_subtree", {
              count: subtreeSize,
            }),
            content: t("chat.message.confirm_delete_subtree_desc"),
            okText: t("common.actions.confirm"),
            cancelText: t("common.actions.cancel"),
            okType: "danger",
            onOk: () => resolve(true),
            onCancel: () => resolve(false),
          });
        });
        if (!confirmed) return;

        await historyPersistQueue.enqueue(async () => {
          await historyService.deleteMessagePersist(msgId);
        });
        await reloadMessages();
        await chatStoreLoadConversations(true);
        message.success(t("chat.message.message_deleted"));
      } catch (e) {
        log.error("Failed to delete message", e);
        message.error(t("chat.message.delete_failed"));
      }
    },
    [chatStoreLoadConversations, t, reloadMessages, modal],
  );

  const deleteConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.deleteConversation(id);
        });
        if (selectedId === id) {
          setSelectedId(null);
          setMessages([]);
          setTotalCount(0);
        }
        await chatStoreLoadConversations(true);
        message.success(t("history.conversation_deleted"));
      } catch (e) {
        log.error("Failed to delete conversation", e);
        notifyError(
          "history.errors.deleteConversationFailed.title",
          "history.errors.deleteConversationFailed.description",
        );
        message.error(t("history.operation_failed"));
      }
    },
    [selectedId, chatStoreLoadConversations, t],
  );

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.renameConversation(id, title, "user");
        });
        await chatStoreLoadConversations(true);
        message.success(t("history.renamed"));
      } catch (e) {
        log.error("Failed to rename", e);
        notifyError(
          "history.errors.renameFailed.title",
          "history.errors.renameFailed.description",
        );
        message.error(t("history.rename_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const exportConversation = useCallback(
    async (id: string, format: string = "markdown") => {
      try {
        const content = await historyService.exportConversation(id, format);
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
        message.success(t("history.export_success"));
      } catch (e) {
        log.error("Export failed", e);
        notifyError(
          "history.errors.exportFailed.title",
          "history.errors.exportFailed.description",
        );
        message.error(t("history.export_failed"));
      }
    },
    [t],
  );

  const togglePin = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.toggleConversationPin(id);
        });
        await chatStoreLoadConversations(true);
      } catch (e) {
        log.error("Failed to toggle pin", e);
        message.error(t("history.operation_failed"));
      }
    },
    [chatStoreLoadConversations, t],
  );

  const duplicateConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.duplicateConversation(id);
        });
        await chatStoreLoadConversations(true);
        message.success(t("history.duplicated"));
      } catch (e) {
        log.error("Failed to duplicate", e);
        message.error(t("history.duplicate_failed"));
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
      {modalContextHolder}
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
        onLoadMore={() => {
          void loadMoreMessages();
        }}
      />
    </div>
  );
};
