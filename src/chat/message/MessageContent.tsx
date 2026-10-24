import React, { useEffect, useRef, useState, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import { parseContent } from "../core/contentBlockUtils";
import { ImageThumbnail } from "./ImageThumbnail";
import { FileCard } from "./FileCard";
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

  const remarkPlugins = useMemo(() => [remarkGfm, remarkMath], []);
  const rehypePlugins = useMemo(() => [rehypeKatex], []);

  if (!content) return null;

  const blocks = parseContent(content);

  return (
    <div className={css.root} data-streaming={markdownStreaming || undefined}>
      {blocks ? (
        blocks.map((block, i) => {
          switch (block.type) {
            case "text":
              return isUser ? (
                <p key={i}>{block.text}</p>
              ) : (
                <ReactMarkdown
                  key={i}
                  remarkPlugins={remarkPlugins}
                  rehypePlugins={rehypePlugins}
                >
                  {block.text}
                </ReactMarkdown>
              );
            case "image":
              return (
                <div key={i} className={css.imageBlock}>
                  <ImageThumbnail
                    attachment={block.attachment}
                    cachedData={block.cached_data}
                    offloaded={block.offloaded}
                  />
                </div>
              );
            case "file":
              return (
                <div key={i} className={css.fileBlock}>
                  <FileCard attachment={block.attachment} />
                </div>
              );
            default:
              return null;
          }
        })
      ) : isUser ? (
        <p>{content}</p>
      ) : (
        <ReactMarkdown
          remarkPlugins={remarkPlugins}
          rehypePlugins={rehypePlugins}
        >
          {content}
        </ReactMarkdown>
      )}
    </div>
  );
};
