import React, { useCallback, useMemo, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Empty, Spin } from "antd";
import { RightOutlined, DownOutlined } from "@ant-design/icons";
import type { Conversation } from "../../chat/core/chatTypes";
import {
  groupConversations,
  type ConversationGroup,
} from "../core/conversationGrouping";
import { ConversationListItem } from "./ConversationListItem";
import {
  HistorySearchBar,
  type HistorySearchBarHandle,
} from "./HistorySearchBar";
import { translateKey } from "../../i18n";
import css from "./HistorySidebar.module.css";

interface HistorySidebarProps {
  conversations: Conversation[];
  isLoadingConversations: boolean;
  selectedId: string | null;
  searchQuery: string;
  searchResults: Conversation[] | null;
  searchFullText: boolean;
  refreshing: boolean;
  collapsedGroups: Set<string>;
  multiSelectMode: boolean;
  selectedConversationIds: Set<string>;
  focusedIndex: number;
  autoFocusSearch: boolean;
  onAutoFocusSearchConsumed: () => void;
  onSearch: (value: string) => void;
  onToggleFullText: () => void;
  onRefresh: () => void;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onExport: (id: string, format: string) => void;
  onTogglePin: (id: string) => void;
  onDuplicate: (id: string) => void;
  onToggleGroup: (key: string) => void;
  onEnterMultiSelect: () => void;
  onExitMultiSelect: () => void;
  onToggleConversationSelection: (id: string) => void;
  onSetFocusedIndex: (index: number) => void;
}
export const HistorySidebar: React.FC<HistorySidebarProps> = React.memo(
  ({
    conversations,
    isLoadingConversations,
    selectedId,
    searchQuery,
    searchResults,
    searchFullText,
    refreshing,
    collapsedGroups,
    multiSelectMode,
    selectedConversationIds,
    focusedIndex,
    autoFocusSearch,
    onAutoFocusSearchConsumed,
    onSearch,
    onToggleFullText,
    onRefresh,
    onSelect,
    onDelete,
    onRename,
    onExport,
    onTogglePin,
    onDuplicate,
    onToggleGroup,
    onEnterMultiSelect,
    onExitMultiSelect,
    onToggleConversationSelection,
    onSetFocusedIndex,
  }) => {
    const { t } = useTranslation();
    const sidebarRef = useRef<HTMLDivElement>(null);
    const searchBarRef = useRef<HistorySearchBarHandle>(null);

    const displayConvs = searchResults ?? conversations;
    const groups = useMemo(
      () => groupConversations(displayConvs, translateKey),
      [displayConvs],
    );

    const flatConvs = useMemo(() => {
      if (searchQuery.trim() && searchResults !== null) {
        return displayConvs;
      }
      return groups.flatMap((g) =>
        collapsedGroups.has(g.key) ? [] : g.conversations,
      );
    }, [searchQuery, searchResults, displayConvs, groups, collapsedGroups]);

    useEffect(() => {
      if (!autoFocusSearch) return;
      searchBarRef.current?.focusSearchInput();
      onAutoFocusSearchConsumed();
    }, [autoFocusSearch, onAutoFocusSearchConsumed]);

    useEffect(() => {
      const el = sidebarRef.current;
      if (!el) return;

      const handleKeyDown = (e: KeyboardEvent) => {
        const target = e.target as HTMLElement;
        const isInput =
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable;

        if ((e.ctrlKey || e.metaKey) && e.key === "f") {
          e.preventDefault();
          searchBarRef.current?.focusSearchInput();
          return;
        }

        if (isInput) return;

        if (flatConvs.length === 0) return;

        switch (e.key) {
          case "ArrowUp": {
            e.preventDefault();
            const next =
              focusedIndex <= 0 ? flatConvs.length - 1 : focusedIndex - 1;
            onSetFocusedIndex(next);
            break;
          }
          case "ArrowDown": {
            e.preventDefault();
            const next =
              focusedIndex >= flatConvs.length - 1 ? 0 : focusedIndex + 1;
            onSetFocusedIndex(next);
            break;
          }
          case "Enter": {
            e.preventDefault();
            if (focusedIndex >= 0 && focusedIndex < flatConvs.length) {
              const conv = flatConvs[focusedIndex];
              if (multiSelectMode) {
                onToggleConversationSelection(conv.id);
              } else {
                onSelect(conv.id);
              }
            }
            break;
          }
          case " ": {
            if (multiSelectMode) {
              e.preventDefault();
              if (focusedIndex >= 0 && focusedIndex < flatConvs.length) {
                onToggleConversationSelection(flatConvs[focusedIndex].id);
              }
            }
            break;
          }
          case "Delete": {
            e.preventDefault();
            if (focusedIndex >= 0 && focusedIndex < flatConvs.length) {
              onDelete(flatConvs[focusedIndex].id);
            }
            break;
          }
          case "Escape": {
            e.preventDefault();
            if (multiSelectMode) {
              onExitMultiSelect();
            } else if (focusedIndex >= 0) {
              onSetFocusedIndex(-1);
            }
            break;
          }
        }
      };

      el.addEventListener("keydown", handleKeyDown);
      return () => el.removeEventListener("keydown", handleKeyDown);
    }, [
      flatConvs,
      focusedIndex,
      multiSelectMode,
      onSetFocusedIndex,
      onToggleConversationSelection,
      onSelect,
      onDelete,
      onExitMultiSelect,
    ]);

    useEffect(() => {
      if (focusedIndex >= 0 && focusedIndex < flatConvs.length) {
        const itemEl = sidebarRef.current?.querySelector(
          `[data-conv-id="${flatConvs[focusedIndex].id}"]`,
        );
        itemEl?.scrollIntoView({ block: "nearest" });
      }
    }, [focusedIndex, flatConvs]);

    const renderItem = useCallback(
      (conv: Conversation, index: number) => (
        <div key={conv.id} data-conv-id={conv.id}>
          <ConversationListItem
            conversation={conv}
            selected={conv.id === selectedId}
            multiSelectMode={multiSelectMode}
            checked={selectedConversationIds.has(conv.id)}
            focused={index === focusedIndex}
            index={index}
            onSelect={onSelect}
            onDelete={onDelete}
            onRename={onRename}
            onExport={onExport}
            onTogglePin={onTogglePin}
            onDuplicate={onDuplicate}
            onToggleCheck={onToggleConversationSelection}
            onSetFocusedIndex={onSetFocusedIndex}
          />
        </div>
      ),
      [
        selectedId,
        multiSelectMode,
        selectedConversationIds,
        focusedIndex,
        onSelect,
        onDelete,
        onRename,
        onExport,
        onTogglePin,
        onDuplicate,
        onToggleConversationSelection,
        onSetFocusedIndex,
      ],
    );

    return (
      <div
        className={css.sidebar}
        id="history-sidebar"
        ref={sidebarRef}
        tabIndex={-1}
      >
        <HistorySearchBar
          ref={searchBarRef}
          searchQuery={searchQuery}
          onSearch={onSearch}
          searchFullText={searchFullText}
          onToggleFullText={onToggleFullText}
          refreshing={refreshing}
          onRefresh={onRefresh}
          multiSelectMode={multiSelectMode}
          onToggleMultiSelect={
            multiSelectMode ? onExitMultiSelect : onEnterMultiSelect
          }
        />
        <div className={css.list}>
          {isLoadingConversations ? (
            <div className={css.loadingWrap}>
              <Spin />
            </div>
          ) : searchQuery.trim() && searchResults !== null ? (
            <>
              <div className={css.filterHint}>
                {t("history.result_count", { count: displayConvs.length })}
              </div>
              {displayConvs.length === 0 ? (
                <Empty
                  description={t("history.no_match")}
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                />
              ) : (
                displayConvs.map((conv, i) => renderItem(conv, i))
              )}
            </>
          ) : groups.length === 0 ? (
            <Empty
              description={t("history.no_conversations")}
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          ) : (
            groups.map((group: ConversationGroup) => (
              <div key={group.key} className={css.group}>
                <div
                  className={css.groupHeader}
                  onClick={() => onToggleGroup(group.key)}
                >
                  <span className={css.groupArrow}>
                    {collapsedGroups.has(group.key) ? (
                      <RightOutlined />
                    ) : (
                      <DownOutlined />
                    )}
                  </span>
                  <span className={css.groupLabel}>{group.label}</span>
                  <span className={css.groupCount}>
                    {group.conversations.length}
                  </span>
                </div>
                {!collapsedGroups.has(group.key) &&
                  group.conversations.map((conv) => {
                    const globalIndex = flatConvs.findIndex(
                      (c) => c.id === conv.id,
                    );
                    return renderItem(conv, globalIndex);
                  })}
              </div>
            ))
          )}
        </div>
      </div>
    );
  },
);
