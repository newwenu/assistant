import React, { useEffect, useRef, useState, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import css from "./MessageContent.module.css";

interface MessageContentProps {
  content: string;
  role: string;
  isStreaming?: boolean;
}

export const MessageContent: React.FC<MessageContentProps> = ({
  content,
  role,
  isStreaming = false,
}) => {
  const isUser = role === "user";

  const [markdownStreaming, setMarkdownStreaming] = useState(isStreaming);
  const streamEndedRef = useRef(false);

  useEffect(() => {
    if (isStreaming) {
      streamEndedRef.current = false;
      setMarkdownStreaming(true);
      return;
    }
    if (streamEndedRef.current) return;
    streamEndedRef.current = true;
    let cancelled = false;
    const frameId = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!cancelled) setMarkdownStreaming(false);
      });
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(frameId);
    };
  }, [isStreaming]);

  const remarkPlugins = useMemo(() => [remarkGfm], []);

  if (!content) return null;

  return (
    <div className={css.root} data-streaming={markdownStreaming || undefined}>
      {isUser ? (
        <p>{content}</p>
      ) : (
        <ReactMarkdown remarkPlugins={remarkPlugins}>{content}</ReactMarkdown>
      )}
    </div>
  );
};
