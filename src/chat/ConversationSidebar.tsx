import React, {
  useState,
  useMemo,
  useCallback,
  useRef,
  useEffect,
} from "react";
import { Input, Button, Modal, Dropdown } from "antd";
import {
  PlusOutlined,
  DeleteOutlined,
  EditOutlined,
  SearchOutlined,
  ExportOutlined,
} from "@ant-design/icons";
import type { Conversation } from "../stores/chatStore";
import { CHAT_SIDEBAR_WIDTH } from "../config/constants";

interface ConversationSidebarProps {
  conversations: Conversation[];
  currentId: string | null;
  providerIds: Set<string>;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onCreate: () => void;
  onSearch: (query: string) => void;
  onClearSearch: () => void;
  searchResults: Conversation[] | null;
  visible: boolean;
}

interface ConversationGroup {
  key: string;
  label: string;
  conversations: Conversation[];
}

function relativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  if (diff < 60000) return "刚刚";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}分钟前`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}小时前`;
  if (diff < 172800000) return "昨天";
  return new Date(timestamp).toLocaleDateString();
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

export const ConversationSidebar: React.FC<ConversationSidebarProps> = ({
  conversations,
  currentId,
  providerIds,
  onSelect,
  onDelete,
  onRename,
  onCreate,
  onSearch,
  onClearSearch,
  searchResults,
  visible,
}) => {
  const [search, setSearch] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const displayConversations =
    searchResults !== null ? searchResults : conversations;

  const groups = useMemo(
    () => groupConversations(displayConversations),
    [displayConversations],
  );

  useEffect(() => {
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    if (!search.trim()) {
      onClearSearch();
      return;
    }
    searchTimerRef.current = setTimeout(() => {
      onSearch(search);
    }, 300);
    return () => {
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    };
  }, [search]);

  const handleRename = useCallback(
    (id: string, title: string) => {
      onRename(id, title);
      setEditingId(null);
    },
    [onRename],
  );

  const toggleGroup = useCallback((key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const renderConversationItem = useCallback(
    (conv: Conversation) => (
      <div
        key={conv.id}
        className={`chat-sidebar-item ${conv.id === currentId ? "active" : ""}`}
        onClick={() => {
          if (editingId !== conv.id) onSelect(conv.id);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
        }}
      >
        {editingId === conv.id ? (
          <Input
            size="small"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onBlur={() => handleRename(conv.id, editTitle)}
            onPressEnter={() => handleRename(conv.id, editTitle)}
            autoFocus
          />
        ) : (
          <>
            <div className="chat-sidebar-item-title">
              {conv.title}
              {conv.provider_id && !providerIds.has(conv.provider_id) && (
                <span className="chat-sidebar-item-badge">Provider 不可用</span>
              )}
              {!conv.provider_id && (
                <span className="chat-sidebar-item-badge">Provider 不可用</span>
              )}
            </div>
            <div className="chat-sidebar-item-time">
              {relativeTime(conv.updated_at)}
            </div>
            <Dropdown
              menu={{
                items: [
                  {
                    key: "rename",
                    icon: <EditOutlined />,
                    label: "重命名",
                    onClick: () => {
                      setEditingId(conv.id);
                      setEditTitle(conv.title);
                    },
                  },
                  {
                    key: "export",
                    icon: <ExportOutlined />,
                    label: "导出",
                    onClick: () => {
                      handleExportConversation(conv);
                    },
                  },
                  { type: "divider" },
                  {
                    key: "delete",
                    icon: <DeleteOutlined />,
                    label: "删除",
                    danger: true,
                    onClick: () => {
                      Modal.confirm({
                        title: "确认删除对话？",
                        content: "删除后无法恢复",
                        onOk: () => onDelete(conv.id),
                      });
                    },
                  },
                ],
              }}
              trigger={["contextMenu"]}
            >
              <span />
            </Dropdown>
          </>
        )}
      </div>
    ),
    [
      currentId,
      editingId,
      editTitle,
      providerIds,
      onSelect,
      onDelete,
      handleRename,
    ],
  );

  return (
    <div
      className="chat-sidebar"
      style={{
        width: visible ? CHAT_SIDEBAR_WIDTH : 0,
        transform: visible ? "translateX(0)" : "translateX(-100%)",
      }}
    >
      <div className="chat-sidebar-header">
        <Input
          placeholder="搜索对话"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          size="small"
          allowClear
          prefix={
            <SearchOutlined
              style={{ color: "var(--color-text-quaternary, #999)" }}
            />
          }
          className="chat-sidebar-search"
        />
      </div>
      <div className="chat-sidebar-list">
        {displayConversations.length === 0 && (
          <div className="chat-sidebar-empty">
            {search ? "未找到匹配的对话" : "暂无对话"}
          </div>
        )}
        {searchResults !== null
          ? displayConversations.map((conv) => renderConversationItem(conv))
          : groups.map((group) => (
              <div key={group.key} className="chat-sidebar-group">
                <div
                  className="chat-sidebar-group-header"
                  onClick={() => toggleGroup(group.key)}
                >
                  <span className="chat-sidebar-group-arrow">
                    {collapsedGroups.has(group.key) ? "▸" : "▾"}
                  </span>
                  <span className="chat-sidebar-group-label">
                    {group.label}
                  </span>
                  <span className="chat-sidebar-group-count">
                    {group.conversations.length}
                  </span>
                </div>
                {!collapsedGroups.has(group.key) &&
                  group.conversations.map((conv) =>
                    renderConversationItem(conv),
                  )}
              </div>
            ))}
      </div>
      <div className="chat-sidebar-footer">
        <Button
          type="primary"
          icon={<PlusOutlined />}
          onClick={onCreate}
          block
          size="small"
        >
          新建对话
        </Button>
      </div>
    </div>
  );
};

async function handleExportConversation(conv: Conversation) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const content = await invoke<string>("export_conversation", {
      id: conv.id,
      format: "markdown",
    });
    const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${conv.title}.md`;
    a.click();
    URL.revokeObjectURL(url);
  } catch (e) {
    console.error("Failed to export conversation:", e);
  }
}
