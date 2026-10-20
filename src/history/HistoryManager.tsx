import React, { useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { App } from "antd";
import { save } from "@tauri-apps/plugin-dialog";
import { historyService } from "../services/historyService";
import { subscribeHistoryEvents } from "../services/historyEvents";
import { useChatStore } from "../stores/chatStore";
import { useHistoryStore } from "../stores/historyStore";
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
  const { message, modal } = App.useApp();
  const conversations = useChatStore((s) => s.conversations);
  const isLoadingConversations = useChatStore((s) => s.isLoadingConversations);

  const selectedId = useHistoryStore((s) => s.selectedId);
  const messages = useHistoryStore((s) => s.messages);
  const totalCount = useHistoryStore((s) => s.totalCount);
  const hasMore = useHistoryStore((s) => s.hasMore);
  const loadingMore = useHistoryStore((s) => s.loadingMore);
  const loadingMessages = useHistoryStore((s) => s.loadingMessages);
  const collapsedGroups = useHistoryStore((s) => s.collapsedGroups);
  const selectConversation = useHistoryStore((s) => s.selectConversation);
  const clearSelection = useHistoryStore((s) => s.clearSelection);
  const refreshMessages = useHistoryStore((s) => s.refreshMessages);
  const loadMoreMessages = useHistoryStore((s) => s.loadMoreMessages);
  const toggleGroup = useHistoryStore((s) => s.toggleGroup);
  const loadConversations = useHistoryStore((s) => s.loadConversations);

  const {
    searchQuery,
    searchResults,
    searchFullText,
    setSearchFullText,
    handleSearch,
  } = useHistorySearch(conversations);

  const { refreshing, handleRefresh } = useHistoryRefresh(refreshMessages);

  useEffect(() => {
    void loadConversations();
  }, [loadConversations]);

  useEffect(() => {
    const cleanup = subscribeHistoryEvents();
    return cleanup;
  }, []);

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

        const hide = message.loading(t("history.deleting_message"), 0);
        try {
          await historyPersistQueue.enqueue(async () => {
            await historyService.deleteMessagePersist(msgId);
          });
          await refreshMessages();
          hide();
          message.success(t("chat.message.message_deleted"));
        } catch (e) {
          hide();
          throw e;
        }
      } catch (e) {
        log.error("Failed to delete message", e);
        message.error(t("chat.message.delete_failed"));
      }
    },
    [t, message, modal, refreshMessages],
  );

  const deleteConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.deleteConversation(id);
        });
        if (selectedId === id) {
          clearSelection();
        }
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
    [selectedId, t, message, clearSelection],
  );

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.renameConversation(id, title, "user");
        });
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
    [t, message],
  );

  const exportConversation = useCallback(
    async (id: string, format: string = "markdown") => {
      const ext = format === "json" ? "json" : "md";
      const filterName = format === "json" ? "JSON" : "Markdown";
      const filePath = await save({
        defaultPath: `conversation.${ext}`,
        filters: [
          { name: filterName, extensions: [ext] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (!filePath) return;
      try {
        await historyService.exportConversationToFile(id, format, filePath);
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
    [t, message],
  );

  const togglePin = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.toggleConversationPin(id);
        });
      } catch (e) {
        log.error("Failed to toggle pin", e);
        message.error(t("history.operation_failed"));
      }
    },
    [t, message],
  );

  const duplicateConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.duplicateConversation(id);
        });
        message.success(t("history.duplicated"));
      } catch (e) {
        log.error("Failed to duplicate", e);
        message.error(t("history.duplicate_failed"));
      }
    },
    [t, message],
  );

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
        onLoadMore={() => {
          void loadMoreMessages();
        }}
      />
    </div>
  );
};
