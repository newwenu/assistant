import React, {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Input,
  Typography,
  Button,
  Dropdown,
  Empty,
  Spin,
  Tag,
  Popconfirm,
  Tooltip,
  message,
} from "antd";
import {
  SearchOutlined,
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  MessageOutlined,
  ClockCircleOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import type { Conversation, Message } from "../stores/chatStore";
import {
  MESSAGES_PAGE_SIZE,
  VIRTUAL_SCROLL_THRESHOLD,
  VIRTUAL_SCROLL_OVERSCAN,
  SCROLL_LOAD_THRESHOLD_PX,
} from "../config/constants";

interface ConversationGroup {
  key: string;
  label: string;
  conversations: Conversation[];
}

function groupConversations(convs: Conversation[]): ConversationGroup[] {
  const groups: ConversationGroup[] = [];

  const now = new Date();
  const todayStart = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
  ).getTime();
  const yesterdayStart = todayStart - 86400000;
  const weekStart = todayStart - 6 * 86400000;
  const monthStart = todayStart - 29 * 86400000;

  const today = convs.filter((c) => c.updated_at >= todayStart);
  const yesterday = convs.filter(
    (c) => c.updated_at >= yesterdayStart && c.updated_at < todayStart,
  );
  const week = convs.filter(
    (c) => c.updated_at >= weekStart && c.updated_at < yesterdayStart,
  );
  const month = convs.filter(
    (c) => c.updated_at >= monthStart && c.updated_at < weekStart,
  );
  const older = convs.filter((c) => c.updated_at < monthStart);

  if (today.length)
    groups.push({ key: "today", label: "今天", conversations: today });
  if (yesterday.length)
    groups.push({ key: "yesterday", label: "昨天", conversations: yesterday });
  if (week.length)
    groups.push({ key: "week", label: "近 7 天", conversations: week });
  if (month.length)
    groups.push({ key: "month", label: "近 30 天", conversations: month });
  if (older.length)
    groups.push({ key: "older", label: "更早", conversations: older });

  return groups;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatRelativeTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60000) return "刚刚";
  if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`;
  if (diff < 172800000) return "昨天";
  return formatTime(ts);
}

const REFRESH_INTERVAL_MS = 15000;

export const HistoryManager: React.FC = () => {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Conversation[] | null>(
    null,
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadingConvList, setLoadingConvList] = useState(true);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const refreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  const loadConversations = useCallback(async () => {
    setLoadingConvList(true);
    try {
      const result = await invoke("list_conversations", { providerId: null });
      setConversations((result as Conversation[]) || []);
    } catch (e) {
      console.error("Failed to load conversations:", e);
    } finally {
      setLoadingConvList(false);
    }
  }, []);

  const refreshConversations = useCallback(async () => {
    setRefreshing(true);
    try {
      const result = await invoke("list_conversations", { providerId: null });
      setConversations((result as Conversation[]) || []);
    } catch (e) {
      console.error("Failed to refresh conversations:", e);
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    loadConversations();
  }, [loadConversations]);

  useEffect(() => {
    refreshTimerRef.current = setInterval(() => {
      refreshConversations();
    }, REFRESH_INTERVAL_MS);
    return () => {
      if (refreshTimerRef.current) clearInterval(refreshTimerRef.current);
    };
  }, [refreshConversations]);

  const handleSearch = useCallback((value: string) => {
    setSearchQuery(value);
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    if (!value.trim()) {
      setSearchResults(null);
      return;
    }
    searchTimerRef.current = setTimeout(async () => {
      try {
        const result = await invoke("search_conversations", { query: value });
        setSearchResults((result as Conversation[]) || []);
      } catch (e) {
        console.error("Search failed:", e);
        setSearchResults([]);
      }
    }, 300);
  }, []);

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

  const deleteMessage = useCallback(async (msgId: string) => {
    try {
      await invoke("delete_message_persist", { messageId: msgId });
      setMessages((prev) => prev.filter((m) => m.id !== msgId));
      setTotalCount((prev) => Math.max(0, prev - 1));
      message.success("消息已删除");
    } catch (e) {
      console.error("Failed to delete message:", e);
      message.error("删除失败");
    }
  }, []);

  const deleteConversation = useCallback(
    async (id: string) => {
      try {
        await invoke("delete_conversation", { id });
        if (selectedId === id) {
          setSelectedId(null);
          setMessages([]);
          setTotalCount(0);
        }
        await loadConversations();
        message.success("对话已删除");
      } catch (e) {
        console.error("Failed to delete conversation:", e);
        message.error("删除失败");
      }
    },
    [selectedId, loadConversations],
  );

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await invoke("rename_conversation", { id, title });
        await loadConversations();
        message.success("已重命名");
      } catch (e) {
        console.error("Failed to rename:", e);
        message.error("重命名失败");
      }
    },
    [loadConversations],
  );

  const exportConversation = useCallback(async (id: string) => {
    try {
      const md = await invoke<string>("export_conversation", {
        id,
        format: "markdown",
      });
      const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `conversation-${id}.md`;
      a.click();
      URL.revokeObjectURL(url);
      message.success("导出成功");
    } catch (e) {
      console.error("Export failed:", e);
      message.error("导出失败");
    }
  }, []);

  const handleScroll = useCallback(() => {
    if (!messageListRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = messageListRef.current;
    setUserScrolledUp(scrollHeight - scrollTop - clientHeight > 100);

    if (hasMore && !loadingMore && scrollTop < SCROLL_LOAD_THRESHOLD_PX) {
      loadMoreMessages();
    }
  }, [hasMore, loadingMore, loadMoreMessages]);

  const scrollToBottom = useCallback(() => {
    if (messageListRef.current) {
      messageListRef.current.scrollTo({
        top: messageListRef.current.scrollHeight,
        behavior: "smooth",
      });
      setUserScrolledUp(false);
    }
  }, []);

  const toggleGroup = useCallback((key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const displayConvs = searchResults ?? conversations;
  const groups = useMemo(
    () => groupConversations(displayConvs),
    [displayConvs],
  );

  const selectedConv = conversations.find((c) => c.id === selectedId);

  const useVirtualScroll = messages.length > VIRTUAL_SCROLL_THRESHOLD;

  const visibleMessages = useMemo(() => {
    if (!useVirtualScroll) return messages;
    const container = messageListRef.current;
    if (!container) return messages;

    const ESTIMATED_ITEM_HEIGHT = 80;
    const { scrollTop, clientHeight } = container;
    const start = Math.max(
      0,
      Math.floor(scrollTop / ESTIMATED_ITEM_HEIGHT) - VIRTUAL_SCROLL_OVERSCAN,
    );
    const end = Math.min(
      messages.length,
      Math.ceil((scrollTop + clientHeight) / ESTIMATED_ITEM_HEIGHT) +
        VIRTUAL_SCROLL_OVERSCAN,
    );
    return messages.slice(start, end);
  }, [messages, useVirtualScroll]);

  return (
    <div
      style={{
        display: "flex",
        height: "100%",
        background: "var(--color-bg-layout)",
      }}
    >
      <div
        style={{
          width: 280,
          flexShrink: 0,
          display: "flex",
          flexDirection: "column",
          background: "var(--color-bg-sidebar)",
          borderRight: "1px solid var(--color-border)",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            padding: "10px 10px 6px",
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <Input
            prefix={<SearchOutlined />}
            placeholder="搜索对话..."
            allowClear
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            size="small"
            style={{ flex: 1 }}
          />
          <Tooltip title="刷新列表">
            <Button
              type="text"
              size="small"
              icon={<ReloadOutlined spin={refreshing} />}
              onClick={refreshConversations}
              style={{ flexShrink: 0 }}
            />
          </Tooltip>
        </div>
        <div
          style={{
            flex: 1,
            overflowY: "auto",
            padding: "0 8px 8px",
            minHeight: 0,
          }}
        >
          {loadingConvList ? (
            <div style={{ textAlign: "center", padding: 24 }}>
              <Spin />
            </div>
          ) : groups.length === 0 ? (
            <Empty
              description="暂无对话"
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          ) : (
            groups.map((group) => (
              <div key={group.key} className="chat-sidebar-group">
                <div
                  className="chat-sidebar-group-header"
                  onClick={() => toggleGroup(group.key)}
                >
                  <span className="chat-sidebar-group-arrow">
                    {collapsedGroups.has(group.key) ? "▶" : "▼"}
                  </span>
                  <span className="chat-sidebar-group-label">
                    {group.label}
                  </span>
                  <span className="chat-sidebar-group-count">
                    {group.conversations.length}
                  </span>
                </div>
                {!collapsedGroups.has(group.key) &&
                  group.conversations.map((conv) => (
                    <ConversationListItem
                      key={conv.id}
                      conversation={conv}
                      selected={conv.id === selectedId}
                      onSelect={selectConversation}
                      onDelete={deleteConversation}
                      onRename={renameConversation}
                      onExport={exportConversation}
                    />
                  ))}
              </div>
            ))
          )}
        </div>
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          background: "var(--color-bg)",
          overflow: "hidden",
          minWidth: 0,
        }}
      >
        {selectedId && selectedConv ? (
          <>
            <div
              style={{
                padding: "8px 16px",
                borderBottom: "1px solid var(--color-border-secondary)",
                display: "flex",
                alignItems: "center",
                gap: 8,
                flexShrink: 0,
              }}
            >
              <Typography.Text strong ellipsis style={{ flex: 1 }}>
                {selectedConv.title}
              </Typography.Text>
              <Tag>{totalCount} 条消息</Tag>
              <Button
                size="small"
                icon={<ExportOutlined />}
                onClick={() => exportConversation(selectedId)}
              >
                导出
              </Button>
              <Popconfirm
                title="确定删除此对话？"
                onConfirm={() => deleteConversation(selectedId)}
              >
                <Button size="small" danger icon={<DeleteOutlined />}>
                  删除对话
                </Button>
              </Popconfirm>
            </div>
            <div
              ref={messageListRef}
              onScroll={handleScroll}
              style={{
                flex: 1,
                overflowY: "auto",
                padding: "8px 16px",
                position: "relative",
                minHeight: 0,
              }}
            >
              {loadingMessages ? (
                <div style={{ textAlign: "center", padding: 48 }}>
                  <Spin />
                </div>
              ) : messages.length === 0 ? (
                <Empty description="暂无消息" />
              ) : (
                <>
                  {loadingMore && (
                    <div style={{ textAlign: "center", padding: 8 }}>
                      <Spin size="small" />
                      <Typography.Text
                        type="secondary"
                        style={{ marginLeft: 8, fontSize: 12 }}
                      >
                        加载中...
                      </Typography.Text>
                    </div>
                  )}
                  {hasMore && !loadingMore && (
                    <div
                      style={{
                        textAlign: "center",
                        padding: "4px 0",
                        fontSize: 12,
                        color: "var(--color-text-tertiary)",
                      }}
                    >
                      ↑ 向上滚动加载更多消息
                    </div>
                  )}
                  {(useVirtualScroll ? visibleMessages : messages).map(
                    (msg) => (
                      <HistoryMessageItem
                        key={msg.id}
                        message={msg}
                        onDelete={deleteMessage}
                      />
                    ),
                  )}
                </>
              )}
              {userScrolledUp && (
                <button
                  onClick={scrollToBottom}
                  style={{
                    position: "sticky",
                    bottom: 12,
                    display: "block",
                    margin: "0 auto",
                    background: "var(--color-bg-elevated)",
                    border: "1px solid var(--color-border)",
                    borderRadius: 20,
                    padding: "4px 12px",
                    fontSize: 12,
                    color: "var(--color-text-secondary)",
                    cursor: "pointer",
                    zIndex: 10,
                    boxShadow: "var(--shadow-orb)",
                  }}
                >
                  ↓ 回到底部
                </button>
              )}
            </div>
          </>
        ) : (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: "var(--color-text-tertiary)",
              gap: 8,
            }}
          >
            <MessageOutlined style={{ fontSize: 48, opacity: 0.3 }} />
            <Typography.Text type="secondary">
              选择左侧对话查看消息历史
            </Typography.Text>
          </div>
        )}
      </div>
    </div>
  );
};

interface ConversationListItemProps {
  conversation: Conversation;
  selected: boolean;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onExport: (id: string) => void;
}

const ConversationListItem: React.FC<ConversationListItemProps> = React.memo(
  ({ conversation, selected, onSelect, onDelete, onRename, onExport }) => {
    const [editing, setEditing] = useState(false);
    const [editTitle, setEditTitle] = useState("");

    const startRename = () => {
      setEditTitle(conversation.title);
      setEditing(true);
    };

    const confirmRename = () => {
      if (editTitle.trim() && editTitle !== conversation.title) {
        onRename(conversation.id, editTitle.trim());
      }
      setEditing(false);
    };

    const menuItems: MenuProps["items"] = [
      { key: "rename", icon: <EditOutlined />, label: "重命名" },
      { key: "export", icon: <ExportOutlined />, label: "导出" },
      { type: "divider" },
      {
        key: "delete",
        icon: <DeleteOutlined />,
        label: "删除",
        danger: true,
      },
    ];

    const handleMenuClick: MenuProps["onClick"] = ({ key }) => {
      if (key === "rename") startRename();
      else if (key === "export") onExport(conversation.id);
      else if (key === "delete") onDelete(conversation.id);
    };

    return (
      <Dropdown
        menu={{ items: menuItems, onClick: handleMenuClick }}
        trigger={["contextMenu"]}
      >
        <div
          onClick={() => onSelect(conversation.id)}
          style={{
            padding: "6px 10px",
            borderRadius: 6,
            cursor: "pointer",
            background: selected ? "var(--color-bg-elevated)" : "transparent",
            borderLeft: selected
              ? "2px solid var(--color-primary)"
              : "2px solid transparent",
            marginBottom: 2,
            transition: "background 0.15s",
          }}
          onMouseEnter={(e) => {
            if (!selected)
              (e.currentTarget as HTMLDivElement).style.background =
                "var(--color-bg-hover)";
          }}
          onMouseLeave={(e) => {
            if (!selected)
              (e.currentTarget as HTMLDivElement).style.background =
                "transparent";
          }}
        >
          {editing ? (
            <Input
              size="small"
              value={editTitle}
              onChange={(e) => setEditTitle(e.target.value)}
              onPressEnter={confirmRename}
              onBlur={confirmRename}
              autoFocus
              onClick={(e) => e.stopPropagation()}
            />
          ) : (
            <>
              <Typography.Text
                ellipsis
                style={{
                  fontSize: 13,
                  display: "block",
                  lineHeight: "18px",
                }}
              >
                {conversation.title || "新对话"}
              </Typography.Text>
              <Typography.Text
                type="secondary"
                style={{ fontSize: 11, lineHeight: "14px" }}
              >
                <ClockCircleOutlined style={{ marginRight: 4, fontSize: 10 }} />
                {formatRelativeTime(conversation.updated_at)}
              </Typography.Text>
            </>
          )}
        </div>
      </Dropdown>
    );
  },
);

interface HistoryMessageItemProps {
  message: Message;
  onDelete: (id: string) => void;
}

const HistoryMessageItem: React.FC<HistoryMessageItemProps> = React.memo(
  ({ message: msg, onDelete }) => {
    const isUser = msg.role === "user";
    const isSystem = msg.role === "system";

    return (
      <div
        style={{
          display: "flex",
          justifyContent: isUser ? "flex-end" : "flex-start",
          marginBottom: 8,
        }}
      >
        <div
          style={{
            maxWidth: "80%",
            padding: "8px 12px",
            borderRadius: isUser ? "12px 12px 2px 12px" : "12px 12px 12px 2px",
            background: isUser
              ? "var(--color-primary-bg, var(--color-primary-bg-hover))"
              : isSystem
                ? "var(--color-bg-hover)"
                : "var(--color-bg-elevated)",
            border: `1px solid ${
              isUser
                ? "var(--color-primary-border, var(--color-primary-hover))"
                : "var(--color-border-secondary)"
            }`,
            fontSize: 13,
            lineHeight: 1.6,
            wordBreak: "break-word",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              marginBottom: 4,
            }}
          >
            <Tag
              color={isUser ? "blue" : isSystem ? "orange" : "default"}
              style={{ fontSize: 10, lineHeight: "16px", padding: "0 4px" }}
            >
              {msg.role}
            </Tag>
            <Typography.Text type="secondary" style={{ fontSize: 10 }}>
              {formatTime(msg.timestamp)}
            </Typography.Text>
            {msg.tokens != null && (
              <Typography.Text type="secondary" style={{ fontSize: 10 }}>
                {msg.tokens} tokens
              </Typography.Text>
            )}
            <Popconfirm
              title="确定删除此消息？"
              onConfirm={() => onDelete(msg.id)}
            >
              <DeleteOutlined
                style={{
                  fontSize: 11,
                  color: "var(--color-text-tertiary)",
                  cursor: "pointer",
                  marginLeft: 4,
                  opacity: 0.6,
                }}
              />
            </Popconfirm>
          </div>
          <div style={{ whiteSpace: "pre-wrap" }}>{msg.content}</div>
        </div>
      </div>
    );
  },
);
