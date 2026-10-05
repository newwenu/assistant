import React, { useEffect, useRef, useState, useCallback } from "react";
import { DownOutlined, LoadingOutlined } from "@ant-design/icons";
import { MessageBubble, RoleContext } from "./MessageBubble";
import type { Message } from "../stores/chatStore";
import css from "./MessageList.module.css";

const SCROLL_THRESHOLD = 100;

interface MessageListProps {
  messages: Message[];
  isStreaming: boolean;
  compact?: boolean;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
  onDeleteMessage?: (id: string) => void;
  onEditMessage?: (id: string, newContent: string) => void;
  onRegenerateMessage?: (id: string) => void;
  onRetryMessage?: () => void;
}

export const MessageList: React.FC<MessageListProps> = ({
  messages,
  isStreaming,
  compact = false,
  hasMore = false,
  isLoadingMore = false,
  onLoadMore,
  onDeleteMessage,
  onEditMessage,
  onRegenerateMessage,
  onRetryMessage,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const [unreadCount, setUnreadCount] = useState(0);
  const lastScrollHeightRef = useRef(0);
  const seenMessageCountRef = useRef(messages.length);
  const rafIdRef = useRef<number | null>(null);
  const loadMoreTriggeredRef = useRef(false);

  useEffect(() => {
    if (userScrolledUp) {
      const delta = messages.length - seenMessageCountRef.current;
      if (delta > 0) setUnreadCount((c) => c + delta);
    }
    seenMessageCountRef.current = messages.length;
  }, [messages.length, userScrolledUp]);

  useEffect(() => {
    if (userScrolledUp || !scrollRef.current) return;
    const el = scrollRef.current;
    const newHeight = el.scrollHeight;
    if (newHeight !== lastScrollHeightRef.current) {
      lastScrollHeightRef.current = newHeight;
      el.scrollTo({
        top: newHeight,
        behavior: isStreaming ? "auto" : "smooth",
      });
    }
  }, [messages, isStreaming, userScrolledUp]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const throttledCheck = () => {
      if (rafIdRef.current !== null) return;
      rafIdRef.current = requestAnimationFrame(() => {
        rafIdRef.current = null;
        if (!scrollRef.current) return;
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
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
      setUserScrolledUp(false);
      setUnreadCount(0);
    }
  }, []);

  const streamingMsgId = messages.find(
    (m) => m.role === "assistant" && isStreaming,
  )?.id;

  const containerClass = [css.container, compact ? css.compact : ""]
    .filter(Boolean)
    .join(" ");

  if (messages.length === 0) {
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
                <span>加载中…</span>
              </span>
            ) : (
              <button
                className={css.loadMoreBtn}
                onClick={onLoadMore}
                type="button"
              >
                加载更早的消息
              </button>
            )}
          </div>
        )}
        {messages.map((msg, index) => (
          <RoleContext.Provider
            key={msg.id}
            value={index > 0 ? messages[index - 1].role : undefined}
          >
            <MessageBubble
              message={msg}
              isStreaming={msg.id === streamingMsgId}
              compact={compact}
              onDelete={onDeleteMessage}
              onEdit={onEditMessage}
              onRegenerate={onRegenerateMessage}
              onRetry={msg.error?.retryable ? onRetryMessage : undefined}
            />
          </RoleContext.Provider>
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
        aria-label="回到底部"
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
