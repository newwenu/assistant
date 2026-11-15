import React, { useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { App } from "antd";
import { save } from "@tauri-apps/plugin-dialog";
import { historyService } from "../services/historyService";
import { chatService } from "../services/chatService";
import { subscribeHistoryEvents } from "../services/historyEvents";
import { useChatStore } from "../stores/chatStore";
import { useHistoryStore } from "../stores/historyStore";
import { useAgentModeStore } from "../stores/agentModeStore";
import { useProviderStore } from "../stores/providerStore";
import { PersistQueue } from "../utils/persistQueue";
import { createLogger } from "@/services/logger";
import { notifyError } from "@/stores/notifyStore";
import { HistorySidebar } from "./sidebar/HistorySidebar";
import { MultiSelectBar } from "./sidebar/MultiSelectBar";
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
  const loadingMessages = useHistoryStore((s) => s.loadingMessages);
  const collapsedGroups = useHistoryStore((s) => s.collapsedGroups);
  const multiSelectMode = useHistoryStore((s) => s.multiSelectMode);
  const selectedConversationIds = useHistoryStore(
    (s) => s.selectedConversationIds,
  );
  const focusedIndex = useHistoryStore((s) => s.focusedIndex);
  const selectConversation = useHistoryStore((s) => s.selectConversation);
  const clearSelection = useHistoryStore((s) => s.clearSelection);
  const refreshMessages = useHistoryStore((s) => s.refreshMessages);
  const toggleGroup = useHistoryStore((s) => s.toggleGroup);
  const loadConversations = useHistoryStore((s) => s.loadConversations);
  const enterMultiSelect = useHistoryStore((s) => s.enterMultiSelect);
  const exitMultiSelect = useHistoryStore((s) => s.exitMultiSelect);
  const toggleConversationSelection = useHistoryStore(
    (s) => s.toggleConversationSelection,
  );
  const selectAllConversations = useHistoryStore(
    (s) => s.selectAllConversations,
  );
  const clearConversationSelections = useHistoryStore(
    (s) => s.clearConversationSelections,
  );
  const setFocusedIndex = useHistoryStore((s) => s.setFocusedIndex);

  const {
    searchQuery,
    searchResults,
    searchFullText,
    setSearchFullText,
    handleSearch,
  } = useHistorySearch(conversations);

  const { refreshing, handleRefresh } = useHistoryRefresh(refreshMessages);

  const loadModes = useAgentModeStore((s) => s.loadModes);
  const loadProviders = useProviderStore((s) => s.loadProviders);

  useEffect(() => {
    void loadConversations();
  }, [loadConversations]);

  // 身份解析依赖模式与 provider 目录（I43 §4.3）：历史页可能被直接进入，
  // 不预先加载会让所有历史消息退化为「未知模式 + 原始 modelId」。
  useEffect(() => {
    void loadModes();
    void loadProviders();
  }, [loadModes, loadProviders]);

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
        await loadConversations();
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
    [selectedId, t, message, clearSelection, loadConversations],
  );

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.renameConversation(id, title, "user");
        });
        await loadConversations();
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
    [t, message, loadConversations],
  );

  const exportConversation = useCallback(
    async (id: string, format: string = "markdown") => {
      const ext =
        format === "json" ? "json" : format === "jsonl" ? "jsonl" : "md";
      const filterName =
        format === "json"
          ? "JSON"
          : format === "jsonl"
            ? "JSON Lines"
            : "Markdown";
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
        await loadConversations();
      } catch (e) {
        log.error("Failed to toggle pin", e);
        message.error(t("history.operation_failed"));
      }
    },
    [t, message, loadConversations],
  );

  const duplicateConversation = useCallback(
    async (id: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.duplicateConversation(id);
        });
        await loadConversations();
        message.success(t("history.duplicated"));
      } catch (e) {
        log.error("Failed to duplicate", e);
        message.error(t("history.duplicate_failed"));
      }
    },
    [t, message, loadConversations],
  );

  const batchDeleteConversations = useCallback(
    async (ids: string[]) => {
      if (ids.length === 0) return;
      try {
        await historyPersistQueue.enqueue(async () => {
          await chatService.batchDeleteConversations(ids);
        });
        if (ids.includes(selectedId ?? "")) {
          clearSelection();
        }
        exitMultiSelect();
        await loadConversations();
        message.success(t("history.batch_deleted", { count: ids.length }));
      } catch (e) {
        log.error("Failed to batch delete", e);
        message.error(t("history.operation_failed"));
      }
    },
    [
      selectedId,
      t,
      message,
      clearSelection,
      exitMultiSelect,
      loadConversations,
    ],
  );

  const batchTogglePin = useCallback(
    async (ids: string[]) => {
      if (ids.length === 0) return;
      try {
        for (const id of ids) {
          await historyPersistQueue.enqueue(async () => {
            await historyService.toggleConversationPin(id);
          });
        }
        await loadConversations();
      } catch (e) {
        log.error("Failed to batch toggle pin", e);
        message.error(t("history.operation_failed"));
      }
    },
    [t, message, loadConversations],
  );

  const batchExportConversations = useCallback(
    async (ids: string[]) => {
      if (ids.length === 0) return;
      try {
        for (const id of ids) {
          const filePath = await save({
            defaultPath: `conversation-${id}.md`,
            filters: [
              { name: "Markdown", extensions: ["md"] },
              { name: "All Files", extensions: ["*"] },
            ],
          });
          if (filePath) {
            await historyService.exportConversationToFile(
              id,
              "markdown",
              filePath,
            );
          }
        }
        message.success(t("history.batch_exported", { count: ids.length }));
      } catch (e) {
        log.error("Failed to batch export", e);
        message.error(t("history.export_failed"));
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
        multiSelectMode={multiSelectMode}
        selectedConversationIds={selectedConversationIds}
        focusedIndex={focusedIndex}
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
        onEnterMultiSelect={enterMultiSelect}
        onExitMultiSelect={exitMultiSelect}
        onToggleConversationSelection={toggleConversationSelection}
        onSetFocusedIndex={setFocusedIndex}
      />
      <HistoryDetail
        selectedId={selectedId}
        selectedConv={selectedConv}
        messages={messages}
        totalCount={totalCount}
        loadingMessages={loadingMessages}
        onExport={(id, format) => void exportConversation(id, format)}
        onDeleteConversation={(id) => void deleteConversation(id)}
        onDeleteMessage={(id) => void deleteMessage(id)}
      />
      {multiSelectMode && (
        <MultiSelectBar
          selectedCount={selectedConversationIds.size}
          totalCount={conversations.length}
          allIds={conversations.map((c) => c.id)}
          selectedIds={selectedConversationIds}
          onBatchDelete={(ids) => void batchDeleteConversations(ids)}
          onBatchTogglePin={(ids) => void batchTogglePin(ids)}
          onBatchExport={(ids) => void batchExportConversations(ids)}
          onSelectAll={selectAllConversations}
          onClearSelections={clearConversationSelections}
          onExit={exitMultiSelect}
        />
      )}
    </div>
  );
};
