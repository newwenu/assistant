import React, { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Empty, Spin } from "antd";
import { RightOutlined, DownOutlined } from "@ant-design/icons";
import type { Conversation } from "../../chat/core/chatTypes";
import {
  groupConversations,
  type ConversationGroup,
} from "../core/conversationGrouping";
import { ConversationListItem } from "./ConversationListItem";
import { HistorySearchBar } from "./HistorySearchBar";
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
  }) => {
    const { t } = useTranslation();

    const displayConvs = searchResults ?? conversations;
    const groups = useMemo(
      () => groupConversations(displayConvs, translateKey),
      [displayConvs],
    );

    const renderItem = useCallback(
      (conv: Conversation) => (
        <ConversationListItem
          key={conv.id}
          conversation={conv}
          selected={conv.id === selectedId}
          onSelect={onSelect}
          onDelete={onDelete}
          onRename={onRename}
          onExport={onExport}
          onTogglePin={onTogglePin}
          onDuplicate={onDuplicate}
        />
      ),
      [
        selectedId,
        onSelect,
        onDelete,
        onRename,
        onExport,
        onTogglePin,
        onDuplicate,
      ],
    );

    return (
      <div className={css.sidebar}>
        <HistorySearchBar
          searchQuery={searchQuery}
          onSearch={onSearch}
          searchFullText={searchFullText}
          onToggleFullText={onToggleFullText}
          refreshing={refreshing}
          onRefresh={onRefresh}
        />
        <div className={css.list}>
          {isLoadingConversations ? (
            <div className={css.loadingWrap}>
              <Spin />
            </div>
          ) : searchQuery.trim() && searchResults !== null ? (
            <>
              <div className={css.filterHint}>
                {t("chat.history.result_count", { count: displayConvs.length })}
              </div>
              {displayConvs.length === 0 ? (
                <Empty
                  description={t("chat.history.no_match")}
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                />
              ) : (
                displayConvs.map((conv) => renderItem(conv))
              )}
            </>
          ) : groups.length === 0 ? (
            <Empty
              description={t("chat.history.no_conversations")}
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
                  group.conversations.map((conv) => renderItem(conv))}
              </div>
            ))
          )}
        </div>
      </div>
    );
  },
);
