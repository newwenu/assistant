import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Typography,
  Button,
  Dropdown,
  Empty,
  Spin,
  Tag,
  Popconfirm,
  Switch,
} from "antd";
import {
  DeleteOutlined,
  ExportOutlined,
  MessageOutlined,
} from "@ant-design/icons";
import type { Conversation, Message } from "../../chat/core/chatTypes";
import { HistoryBranchView } from "./HistoryBranchView";
import { formatConversationTitle } from "../core/formatConversationTitle";
import css from "./HistoryDetail.module.css";

interface HistoryDetailProps {
  selectedId: string | null;
  selectedConv: Conversation | undefined;
  messages: Message[];
  totalCount: number;
  loadingMessages: boolean;
  onExport: (id: string, format: string) => void;
  onDeleteConversation: (id: string) => void;
  onDeleteMessage: (id: string) => void;
}

export const HistoryDetail: React.FC<HistoryDetailProps> = React.memo(
  ({
    selectedId,
    selectedConv,
    messages,
    totalCount,
    loadingMessages,
    onExport,
    onDeleteConversation,
    onDeleteMessage,
  }) => {
    const { t } = useTranslation();
    const messageListRef = useRef<HTMLDivElement>(null);
    const autoScrolledRef = useRef<string | null>(null);
    const [userScrolledUp, setUserScrolledUp] = useState(false);
    const [scrollTop, setScrollTop] = useState(0);
    const [viewportHeight, setViewportHeight] = useState(0);
    const [focusLeafId, setFocusLeafId] = useState<string | null>(null);
    const [revealNonce, setRevealNonce] = useState(0);
    const [expandedKeys, setExpandedKeys] = useState<ReadonlySet<string>>(
      () => new Set<string>(),
    );
    const [showToolRows, setShowToolRows] = useState(false);

    useEffect(() => {
      setFocusLeafId(null);
      setExpandedKeys(new Set<string>());
      setShowToolRows(false);
      setScrollTop(0);
      setUserScrolledUp(false);
      autoScrolledRef.current = null;
    }, [selectedId]);

    useEffect(() => {
      const element = messageListRef.current;
      if (!element) return;
      const update = () => setViewportHeight(element.clientHeight);
      update();
      const observer = new ResizeObserver(update);
      observer.observe(element);
      return () => observer.disconnect();
    }, [selectedId]);

    useEffect(() => {
      if (!selectedId || loadingMessages || messages.length === 0) return;
      if (autoScrolledRef.current === selectedId) return;
      autoScrolledRef.current = selectedId;
      const frame = requestAnimationFrame(() => {
        const element = messageListRef.current;
        if (!element) return;
        element.scrollTop = element.scrollHeight;
        setScrollTop(element.scrollTop);
      });
      return () => cancelAnimationFrame(frame);
    }, [selectedId, loadingMessages, messages.length]);

    const handleScroll = useCallback(() => {
      const element = messageListRef.current;
      if (!element) return;
      setScrollTop(element.scrollTop);
      setUserScrolledUp(
        element.scrollHeight - element.scrollTop - element.clientHeight > 100,
      );
    }, []);

    const scrollToBottom = useCallback(() => {
      const element = messageListRef.current;
      if (!element) return;
      element.scrollTo({ top: element.scrollHeight, behavior: "smooth" });
      setUserScrolledUp(false);
    }, []);

    const handleFocusNode = useCallback((nodeId: string) => {
      setFocusLeafId(nodeId);
      setRevealNonce((value) => value + 1);
    }, []);

    const toggleStub = useCallback((key: string) => {
      setExpandedKeys((previous) => {
        const next = new Set(previous);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        return next;
      });
    }, []);

    const collapseAll = useCallback(() => {
      setExpandedKeys(new Set<string>());
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
          <Dropdown
            trigger={["click"]}
            menu={{
              items: [
                {
                  key: "markdown",
                  label: t("history.export_markdown"),
                },
                { key: "json", label: t("history.export_json") },
                { key: "jsonl", label: t("history.export_jsonl") },
              ],
              onClick: ({ key }) => onExport(selectedId, key),
            }}
          >
            <Button size="small" icon={<ExportOutlined />}>
              {t("common.actions.export")}
            </Button>
          </Dropdown>
          <Popconfirm
            title={t("history.confirm_delete_conversation")}
            onConfirm={() => onDeleteConversation(selectedId)}
          >
            <Button size="small" danger icon={<DeleteOutlined />}>
              {t("history.delete_conversation")}
            </Button>
          </Popconfirm>
        </div>
        {messages.length > 0 && (
          <div className={css.toolbar}>
            <span className={css.toolbarLabel}>
              {t("history.detail.show_tool_rows")}
            </span>
            <Switch
              size="small"
              checked={showToolRows}
              onChange={(checked) => setShowToolRows(checked)}
            />
            {expandedKeys.size > 0 && (
              <Button type="link" size="small" onClick={collapseAll}>
                {t("history.tree.collapse_all")}
              </Button>
            )}
          </div>
        )}
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
            <HistoryBranchView
              conversationId={selectedId}
              messages={messages}
              activeBranchId={selectedConv.activeBranchId}
              focusLeafId={focusLeafId}
              expandedKeys={expandedKeys}
              showToolRows={showToolRows}
              scrollTop={scrollTop}
              viewportHeight={viewportHeight}
              scrollContainerRef={messageListRef}
              revealNonce={revealNonce}
              onDeleteMessage={onDeleteMessage}
              onFocusNode={handleFocusNode}
              onToggleStub={toggleStub}
            />
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
