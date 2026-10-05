import React, { useEffect, useRef, useState, useCallback } from "react";
import { MessageBubble, RoleContext } from "./MessageBubble";
import { TemplateCards } from "./TemplateCards";
import type { Message } from "../stores/chatStore";
import type { PromptTemplate } from "./TemplateCards";
import css from "./MessageList.module.css";

interface MessageListProps {
  messages: Message[];
  isStreaming: boolean;
  templates: PromptTemplate[];
  onTemplateSelect: (content: string) => void;
  compact?: boolean;
  onDeleteMessage?: (id: string) => void;
  onRetryMessage?: () => void;
}

export const MessageList: React.FC<MessageListProps> = ({
  messages,
  isStreaming,
  templates,
  onTemplateSelect,
  compact = false,
  onDeleteMessage,
  onRetryMessage,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const lastScrollHeightRef = useRef(0);

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

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
    setUserScrolledUp(distanceFromBottom > 100);
  }, []);

  const scrollToBottom = useCallback(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
      setUserScrolledUp(false);
    }
  }, []);

  const streamingMsgId = messages.find(
    (m) => m.role === "assistant" && isStreaming,
  )?.id;

  if (messages.length === 0) {
    return (
      <div
        className={[css.container, compact ? css.compact : ""]
          .filter(Boolean)
          .join(" ")}
        ref={scrollRef}
      >
        <TemplateCards templates={templates} onSelect={onTemplateSelect} />
      </div>
    );
  }

  return (
    <div
      className={[css.container, compact ? css.compact : ""]
        .filter(Boolean)
        .join(" ")}
      ref={scrollRef}
      onScroll={handleScroll}
    >
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
            onRetry={msg.error?.retryable ? onRetryMessage : undefined}
          />
        </RoleContext.Provider>
      ))}
      {userScrolledUp && (
        <button className={css.scrollToBottom} onClick={scrollToBottom}>
          ↓ 回到底部
        </button>
      )}
    </div>
  );
};
