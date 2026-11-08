import React, {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
} from "react";
import { DownOutlined, LoadingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { MessageBubble, RoleContext } from "./MessageBubble";
import type { Message } from "../../chat/core/chatTypes";
import type { RetryNotification } from "../../chat/core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import css from "./MessageList.module.css";

const SCROLL_THRESHOLD = 100;

interface MessageListProps {
  messages: Message[];
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
}

export const MessageList: React.FC<MessageListProps> = ({
  messages,
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

  const visibleMessages = useMemo(
    () => messages.filter((msg) => msg.role !== "tool"),
    [messages],
  );
  const seenMessageCountRef = useRef(visibleMessages.length);

  const lastAssistantIndex = useMemo(() => {
    for (let i = visibleMessages.length - 1; i >= 0; i--) {
      if (visibleMessages[i].role === "assistant") return i;
    }
    return -1;
  }, [visibleMessages]);

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

  const streamingMsgId =
    streamingMessageId ??
    (visibleMessages.length > 0 && isStreaming
      ? visibleMessages[visibleMessages.length - 1].role === "assistant"
        ? visibleMessages[visibleMessages.length - 1].id
        : undefined
      : undefined);

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
        {visibleMessages.map((msg, index) => {
          const isStreamingMsg = msg.id === streamingMsgId;
          const displayMsg =
            isStreamingMsg && streamingContent != null
              ? { ...msg, content: streamingContent }
              : msg;
          const isLastAssistant = index === lastAssistantIndex;
          return (
            <RoleContext.Provider
              key={msg.id}
              value={index > 0 ? visibleMessages[index - 1].role : undefined}
            >
              <MessageBubble
                message={displayMsg}
                isStreaming={isStreamingMsg}
                isReasoningStreaming={isStreamingMsg && isReasoningStreaming}
                retryStatus={isStreamingMsg ? retryStatus : null}
                compact={compact}
                onDelete={onDeleteMessage}
                onEdit={onEditMessage}
                onRegenerate={isLastAssistant ? onRegenerateMessage : undefined}
                onRetry={
                  msg.error?.retryable
                    ? () => onRetryMessage?.(msg.id)
                    : undefined
                }
                onSwitchBranch={onSwitchBranch}
                onFork={onFork}
                onErrorAction={onErrorAction}
              />
            </RoleContext.Provider>
          );
        })}
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
