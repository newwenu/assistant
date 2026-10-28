import React, { useRef, useState, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Typography, Button, Empty, Spin, Tag, Popconfirm } from "antd";
import {
  DeleteOutlined,
  ExportOutlined,
  MessageOutlined,
} from "@ant-design/icons";
import type { Conversation, Message } from "../../chat/core/chatTypes";
import {
  VIRTUAL_SCROLL_THRESHOLD,
  VIRTUAL_SCROLL_OVERSCAN,
  SCROLL_LOAD_THRESHOLD_PX,
} from "../../config/constants";
import { HistoryMessageItem } from "./HistoryMessageItem";
import { formatConversationTitle } from "../../utils/formatConversationTitle";
import css from "./HistoryDetail.module.css";

interface BranchSlot {
  siblings: Message[];
  activeIndex: number;
  branchLabel: string;
}

function buildBranchView(messages: Message[]): BranchSlot[] {
  if (messages.length === 0) return [];

  const byId = new Map<string, Message>();
  for (const msg of messages) {
    byId.set(msg.id, msg);
  }

  const childrenOf = new Map<string, Message[]>();
  const roots: Message[] = [];

  for (const msg of messages) {
    const pid = msg.parentId;
    if (pid && byId.has(pid)) {
      let siblings = childrenOf.get(pid);
      if (!siblings) {
        siblings = [];
        childrenOf.set(pid, siblings);
      }
      siblings.push(msg);
    } else {
      roots.push(msg);
    }
  }

  const sortSiblings = (siblings: Message[]) =>
    siblings.sort((a, b) => a.siblingIndex - b.siblingIndex);
  sortSiblings(roots);
  for (const siblings of childrenOf.values()) {
    sortSiblings(siblings);
  }

  const depthOf = new Map<string, number>();
  const bfsQueue: string[] = [];
  for (const root of roots) {
    depthOf.set(root.id, 0);
    bfsQueue.push(root.id);
  }
  while (bfsQueue.length > 0) {
    const id = bfsQueue.shift()!;
    const depth = depthOf.get(id)!;
    const children = childrenOf.get(id);
    if (children) {
      for (const child of children) {
        depthOf.set(child.id, depth + 1);
        bfsQueue.push(child.id);
      }
    }
  }

  const byDepth = new Map<number, Message[]>();
  for (const msg of messages) {
    const depth = depthOf.get(msg.id) ?? 0;
    let group = byDepth.get(depth);
    if (!group) {
      group = [];
      byDepth.set(depth, group);
    }
    group.push(msg);
  }

  const depths = Array.from(byDepth.keys()).sort((a, b) => a - b);
  const slots: BranchSlot[] = [];
  let branchCounter = 1;

  for (const depth of depths) {
    const group = byDepth.get(depth)!;

    if (depth === 0) {
      const userRoots = group.filter((m) => m.role === "user");
      const orphanRoots = group.filter((m) => m.role !== "user");

      if (userRoots.length > 0) {
        slots.push({
          siblings: userRoots,
          activeIndex: userRoots.length - 1,
          branchLabel: `B${branchCounter++}`,
        });
      }
      if (orphanRoots.length > 0) {
        slots.push({
          siblings: orphanRoots,
          activeIndex: orphanRoots.length - 1,
          branchLabel: `O1`,
        });
      }
    } else {
      slots.push({
        siblings: group,
        activeIndex: group.length - 1,
        branchLabel: `B${branchCounter++}`,
      });
    }
  }

  return slots;
}

interface HistoryDetailProps {
  selectedId: string | null;
  selectedConv: Conversation | undefined;
  messages: Message[];
  totalCount: number;
  hasMore: boolean;
  loadingMore: boolean;
  loadingMessages: boolean;
  onExport: (id: string, format: string) => void;
  onDeleteConversation: (id: string) => void;
  onDeleteMessage: (id: string) => void;
  onLoadMore: () => void;
}

export const HistoryDetail: React.FC<HistoryDetailProps> = React.memo(
  ({
    selectedId,
    selectedConv,
    messages,
    totalCount,
    hasMore,
    loadingMore,
    loadingMessages,
    onExport,
    onDeleteConversation,
    onDeleteMessage,
    onLoadMore,
  }) => {
    const { t } = useTranslation();
    const messageListRef = useRef<HTMLDivElement>(null);
    const [userScrolledUp, setUserScrolledUp] = useState(false);

    const branchSlots = useMemo(() => buildBranchView(messages), [messages]);

    const useVirtualScroll = branchSlots.length > VIRTUAL_SCROLL_THRESHOLD;

    const visibleSlots = useMemo(() => {
      if (!useVirtualScroll) return branchSlots;
      const container = messageListRef.current;
      if (!container) return branchSlots;

      const ESTIMATED_ITEM_HEIGHT = 80;
      const { scrollTop, clientHeight } = container;
      const start = Math.max(
        0,
        Math.floor(scrollTop / ESTIMATED_ITEM_HEIGHT) - VIRTUAL_SCROLL_OVERSCAN,
      );
      const end = Math.min(
        branchSlots.length,
        Math.ceil((scrollTop + clientHeight) / ESTIMATED_ITEM_HEIGHT) +
          VIRTUAL_SCROLL_OVERSCAN,
      );
      return branchSlots.slice(start, end);
    }, [branchSlots, useVirtualScroll]);

    const handleScroll = useCallback(() => {
      if (!messageListRef.current) return;
      const { scrollTop, scrollHeight, clientHeight } = messageListRef.current;
      setUserScrolledUp(scrollHeight - scrollTop - clientHeight > 100);

      if (hasMore && !loadingMore && scrollTop < SCROLL_LOAD_THRESHOLD_PX) {
        onLoadMore();
      }
    }, [hasMore, loadingMore, onLoadMore]);

    const scrollToBottom = useCallback(() => {
      if (messageListRef.current) {
        messageListRef.current.scrollTo({
          top: messageListRef.current.scrollHeight,
          behavior: "smooth",
        });
        setUserScrolledUp(false);
      }
    }, []);

    if (!selectedId || !selectedConv) {
      return (
        <div className={css.detail}>
          <div className={css.emptyState}>
            <MessageOutlined style={{ fontSize: 48, opacity: 0.3 }} />
            <Typography.Text type="secondary">
              {t("history.select_to_view")}
            </Typography.Text>
          </div>
        </div>
      );
    }

    return (
      <div className={css.detail}>
        <div className={css.header}>
          <Typography.Text
            strong
            ellipsis
            className={css.headerTitle}
            style={{ display: "block" }}
          >
            {formatConversationTitle(selectedConv)}
          </Typography.Text>
          <Tag>{t("history.message_count", { count: totalCount })}</Tag>
          <Button
            size="small"
            icon={<ExportOutlined />}
            onClick={() => onExport(selectedId, "markdown")}
          >
            {t("common.actions.export")}
          </Button>
          <Popconfirm
            title={t("history.confirm_delete_conversation")}
            onConfirm={() => onDeleteConversation(selectedId)}
          >
            <Button size="small" danger icon={<DeleteOutlined />}>
              {t("history.delete_conversation")}
            </Button>
          </Popconfirm>
        </div>
        <div
          ref={messageListRef}
          onScroll={handleScroll}
          className={css.messageList}
        >
          {loadingMessages ? (
            <div className={css.loadingWrap}>
              <Spin />
            </div>
          ) : messages.length === 0 ? (
            <Empty description={t("history.no_messages")} />
          ) : (
            <>
              {loadingMore && (
                <div className={css.loadingMoreWrap}>
                  <Spin size="small" />
                  <Typography.Text
                    type="secondary"
                    style={{ marginLeft: 8, fontSize: 12 }}
                  >
                    {t("common.actions.loading")}
                  </Typography.Text>
                </div>
              )}
              {hasMore && !loadingMore && (
                <div className={css.loadMoreHint}>
                  {t("history.scroll_load_more")}
                </div>
              )}
              {(useVirtualScroll ? visibleSlots : branchSlots).map((slot) => (
                <HistoryMessageItem
                  key={slot.siblings[slot.activeIndex].id}
                  siblings={slot.siblings}
                  activeIndex={slot.activeIndex}
                  branchLabel={slot.branchLabel}
                  onDelete={onDeleteMessage}
                />
              ))}
            </>
          )}
          {userScrolledUp && (
            <button className={css.scrollToBottom} onClick={scrollToBottom}>
              {t("history.back_to_bottom")}
            </button>
          )}
        </div>
      </div>
    );
  },
);
