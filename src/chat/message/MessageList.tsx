import React, {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
} from "react";
import { DownOutlined, LoadingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { TurnGroup } from "./TurnGroup";
import { groupNodeRows } from "./groupNodeRows";
import { useChatStreamStore } from "../../stores/chatStreamStore";
import type { Message } from "../../chat/core/chatTypes";
import type { RetryNotification } from "../../chat/core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import css from "./MessageList.module.css";

const SCROLL_THRESHOLD = 100;

interface MessageListProps {
  messages: Message[];
  /** 只读快照：仅写入 `NodeGroupView.link`，不参与分组或渲染决策（I46 门禁 16）。 */
  link: "chat" | "agent";
  isStreaming: boolean;
  isReasoningStreaming?: boolean;
  streamingMessageId?: string | null;
  streamingContent?: string;
  compact?: boolean;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
  onDeleteMessage?: (id: string) => void;
  onEditMessage?: (id: string, newContent: string) => void;
  onRegenerateMessage?: (id: string) => void;
  onRetryMessage?: (messageId: string) => void;
  onSwitchBranch?: (messageId: string, direction: "prev" | "next") => void;
  onFork?: (messageId: string) => void;
  onErrorAction?: (code: RecoveryActionCode) => void;
  retryStatus?: RetryNotification | null;
  /** 文档面板当前打开的文件路径；`null` 时工具结果恒走卡片（I46 compact §3.7.4）。 */
  docPanelPath?: string | null;
  onOpenDocument?: (path: string, line: number) => void;
}

export const MessageList: React.FC<MessageListProps> = ({
  messages,
  link,
  isStreaming,
  isReasoningStreaming = false,
  streamingMessageId,
  streamingContent,
  compact = false,
  hasMore = false,
  isLoadingMore = false,
  onLoadMore,
  onDeleteMessage,
  onEditMessage,
  onRegenerateMessage,
  onRetryMessage,
  onSwitchBranch,
  onFork,
  onErrorAction,
  retryStatus,
  docPanelPath = null,
  onOpenDocument,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const { t } = useTranslation();
  const [unreadCount, setUnreadCount] = useState(0);
  const lastScrollHeightRef = useRef(0);
  const rafIdRef = useRef<number | null>(null);
  const loadMoreTriggeredRef = useRef(false);
  const programScrollRef = useRef(false);
  const prevIsStreamingRef = useRef(isStreaming);

  // 防御性过滤：结果正文已挂在 assistant 行的 `toolCalls[].result`，
  // tool 行在 wire 上 `content` 恒为空串，仅承担分支拓扑（I46 §2.4 D4）。
  const visibleMessages = useMemo(
    () => messages.filter((msg) => msg.role !== "tool"),
    [messages],
  );
  const seenMessageCountRef = useRef(visibleMessages.length);

  const lastAssistantId = useMemo(() => {
    for (let i = visibleMessages.length - 1; i >= 0; i--) {
      if (visibleMessages[i].role === "assistant") return visibleMessages[i].id;
    }
    return null;
  }, [visibleMessages]);

  const prevRoleById = useMemo(() => {
    const map = new Map<string, string | undefined>();
    visibleMessages.forEach((msg, index) => {
      map.set(msg.id, index > 0 ? visibleMessages[index - 1].role : undefined);
    });
    return map;
  }, [visibleMessages]);

  const currentTurnIndex = useChatStreamStore((s) => s.currentTurnIndex);

  /**
   * 流式占位行：父级未下发 `streamingMessageId` 时退回「末条 assistant」，
   * 供投影兜住 `collecting` 判定（I46 §5.3），避免运行中的 turn 被提前收起。
   */
  const streamingMsgId = useMemo(() => {
    if (streamingMessageId != null) return streamingMessageId;
    if (!isStreaming || visibleMessages.length === 0) return null;
    const last = visibleMessages[visibleMessages.length - 1];
    return last.role === "assistant" ? last.id : null;
  }, [streamingMessageId, isStreaming, visibleMessages]);

  /**
   * 分组投影（I46 §5）：分组只看 `turnIndex`，相位只来自
   * `session-state-changed` 的 `isStreaming` 与 `chat-stream-start` 的 `currentTurnIndex`。
   */
  const groups = useMemo(
    () =>
      groupNodeRows({
        messages: visibleMessages,
        link,
        phase: isStreaming ? "collecting" : "settled",
        currentTurnIndex,
        streamingMessageId: streamingMsgId,
      }),
    [visibleMessages, link, isStreaming, currentTurnIndex, streamingMsgId],
  );

  const handlers = useMemo(
    () => ({
      onDelete: onDeleteMessage,
      onEdit: onEditMessage,
      onRegenerate: onRegenerateMessage,
      onRetryMessage,
      onSwitchBranch,
      onFork,
      onErrorAction,
    }),
    [
      onDeleteMessage,
      onEditMessage,
      onRegenerateMessage,
      onRetryMessage,
      onSwitchBranch,
      onFork,
      onErrorAction,
    ],
  );

  useEffect(() => {
    if (userScrolledUp) {
      const delta = visibleMessages.length - seenMessageCountRef.current;
      if (delta > 0) setUnreadCount((c) => c + delta);
    }
    seenMessageCountRef.current = visibleMessages.length;
  }, [visibleMessages.length, userScrolledUp]);

  useEffect(() => {
    if (userScrolledUp || !scrollRef.current) return;
    const el = scrollRef.current;
    const newHeight = el.scrollHeight;
    if (newHeight !== lastScrollHeightRef.current) {
      lastScrollHeightRef.current = newHeight;
      programScrollRef.current = true;
      el.scrollTo({
        top: newHeight,
        behavior: isStreaming ? "auto" : "smooth",
      });
    }
  }, [visibleMessages, isStreaming, streamingContent, userScrolledUp]);

  useEffect(() => {
    if (prevIsStreamingRef.current && !isStreaming) {
      prevIsStreamingRef.current = isStreaming;
      if (!scrollRef.current) return;
      const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
      const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
      if (distanceFromBottom <= SCROLL_THRESHOLD) {
        setUserScrolledUp(false);
        setUnreadCount(0);
      }
    } else {
      prevIsStreamingRef.current = isStreaming;
    }
  }, [isStreaming]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const throttledCheck = () => {
      if (rafIdRef.current !== null) return;
      rafIdRef.current = requestAnimationFrame(() => {
        rafIdRef.current = null;
        if (!scrollRef.current) return;
        if (programScrollRef.current) {
          programScrollRef.current = false;
          return;
        }
        const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
        const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
        const isUp = distanceFromBottom > SCROLL_THRESHOLD;
        setUserScrolledUp((prev) => {
          if (prev === isUp) return prev;
          return isUp;
        });

        if (
          hasMore &&
          !isLoadingMore &&
          onLoadMore &&
          scrollTop < SCROLL_THRESHOLD &&
          !loadMoreTriggeredRef.current
        ) {
          loadMoreTriggeredRef.current = true;
          onLoadMore();
          setTimeout(() => {
            loadMoreTriggeredRef.current = false;
          }, 1000);
        }
      });
    };
    el.addEventListener("scroll", throttledCheck, { passive: true });
    return () => {
      el.removeEventListener("scroll", throttledCheck);
      if (rafIdRef.current !== null) cancelAnimationFrame(rafIdRef.current);
    };
  }, [hasMore, isLoadingMore, onLoadMore]);

  const scrollToBottom = useCallback(() => {
    if (scrollRef.current) {
      programScrollRef.current = true;
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
      setUserScrolledUp(false);
      setUnreadCount(0);
    }
  }, []);

  const containerClass = [css.container, compact ? css.compact : ""]
    .filter(Boolean)
    .join(" ");

  if (visibleMessages.length === 0) {
    return (
      <div className={css.wrapper}>
        <div className={containerClass} ref={scrollRef} />
      </div>
    );
  }

  return (
    <div className={css.wrapper}>
      <div className={containerClass} ref={scrollRef}>
        {hasMore && (
          <div className={css.loadMoreArea}>
            {isLoadingMore ? (
              <span className={css.loadMoreSpinner}>
                <LoadingOutlined style={{ fontSize: 14 }} />
                <span>{t("chat.message.loading_more")}</span>
              </span>
            ) : (
              <button
                className={css.loadMoreBtn}
                onClick={onLoadMore}
                type="button"
              >
                {t("chat.message.load_earlier")}
              </button>
            )}
          </div>
        )}
        {groups.map((group) => (
          <TurnGroup
            key={
              group.turnIndex !== null
                ? `turn:${group.turnIndex}`
                : `unresolved:${
                    group.nodes[0]?.kind === "message" ||
                    group.nodes[0]?.kind === "identity"
                      ? group.nodes[0].message.id
                      : group.nodes[0]?.kind === "tool"
                        ? group.nodes[0].callId
                        : group.nodes[0]?.messageId
                  }`
            }
            group={group}
            handlers={handlers}
            prevRoleById={prevRoleById}
            isStreaming={isStreaming}
            isReasoningStreaming={isReasoningStreaming}
            streamingMessageId={streamingMsgId ?? null}
            streamingContent={streamingContent ?? null}
            retryStatus={retryStatus}
            compact={compact}
            lastAssistantId={lastAssistantId}
            docPanelPath={docPanelPath}
            onOpenDocument={onOpenDocument}
          />
        ))}
      </div>
      <button
        className={[
          css.scrollToBottom,
          userScrolledUp ? css.scrollToBottomVisible : "",
        ]
          .filter(Boolean)
          .join(" ")}
        onClick={scrollToBottom}
        aria-hidden={!userScrolledUp}
        aria-label={t("chat.message.scroll_to_bottom")}
        tabIndex={userScrolledUp ? 0 : -1}
      >
        <DownOutlined style={{ fontSize: 14 }} />
        {unreadCount > 0 && (
          <span className={css.unreadBadge}>
            {unreadCount > 99 ? "99+" : unreadCount}
          </span>
        )}
      </button>
    </div>
  );
};
