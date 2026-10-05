import React, { useCallback } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  CopyOutlined,
  DeleteOutlined,
  RedoOutlined,
  ToolOutlined,
} from "@ant-design/icons";
import type { Message, ToolCall } from "../stores/chatStore";
import {
  type ChatError,
  ChatErrorCode,
  ERROR_DISPLAY,
  formatRetryAfter,
} from "./chatErrors";
import css from "./MessageBubble.module.css";

interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  compact?: boolean;
  onDelete?: (id: string) => void;
  onRetry?: () => void;
}

function ErrorBlock({
  error,
  onRetry,
}: {
  error: ChatError;
  onRetry?: () => void;
}) {
  const display =
    ERROR_DISPLAY[error.code] ?? ERROR_DISPLAY[ChatErrorCode.InternalError];
  const retryHint = formatRetryAfter(error.retry_after_ms);

  return (
    <div className={css.errorBlock}>
      <div className={css.errorHeader}>
        <span className={css.errorIcon}>{display.icon}</span>
        <span className={css.errorTitle}>{display.title}</span>
      </div>
      <div className={css.errorMessage}>{error.message}</div>
      {retryHint && <div className={css.errorRetryHint}>{retryHint}</div>}
      {error.retryable && onRetry && (
        <button className={css.errorRetryBtn} onClick={onRetry} type="button">
          <RedoOutlined style={{ fontSize: 12, marginRight: 4 }} />
          重试
        </button>
      )}
    </div>
  );
}

function ReasoningBlock({
  content,
  isStreaming,
}: {
  content: string;
  isStreaming?: boolean;
}) {
  const [expanded, setExpanded] = React.useState(false);

  return (
    <div className={css.reasoningBlock}>
      <button
        className={css.reasoningToggle}
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <span className={css.reasoningIcon}>💭</span>
        <span>{expanded ? "收起思考" : "展开思考"}</span>
        {isStreaming && <span className={css.reasoningStreaming}>思考中…</span>}
      </button>
      {expanded && (
        <div className={css.reasoningContent}>
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
        </div>
      )}
    </div>
  );
}

function ToolCallBlock({ toolCall }: { toolCall: ToolCall }) {
  const [expanded, setExpanded] = React.useState(false);
  const argsPreview =
    toolCall.arguments.length > 120
      ? toolCall.arguments.slice(0, 120) + "…"
      : toolCall.arguments;

  return (
    <div className={css.toolCallBlock}>
      <button
        className={css.toolCallToggle}
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <ToolOutlined style={{ fontSize: 12, marginRight: 4 }} />
        <span className={css.toolCallName}>{toolCall.name || toolCall.id}</span>
      </button>
      {expanded && (
        <pre className={css.toolCallArgs}>
          {tryFormatJson(toolCall.arguments) || argsPreview}
        </pre>
      )}
    </div>
  );
}

function tryFormatJson(text: string): string | null {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return null;
  }
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({
  message,
  isStreaming = false,
  compact = false,
  onDelete,
  onRetry,
}) => {
  const isUser = message.role === "user";
  const hasError = message.error != null;
  const hasReasoning = !!message.reasoning;
  const hasToolCalls = !!message.toolCalls && message.toolCalls.length > 0;

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content).catch(() => {});
  }, [message.content]);

  const handleDelete = useCallback(() => {
    onDelete?.(message.id);
  }, [onDelete, message.id]);

  const prevRole = React.useContext(RoleContext);
  const isGroupGap = prevRole !== undefined && prevRole !== message.role;

  const rootClass = [
    css.root,
    isUser ? css.rootRoleUser : css.rootRoleAssistant,
    isGroupGap ? css.rootGroupGap : "",
    compact ? css.compact : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={rootClass} data-role={message.role}>
      <div className={css.stack}>
        <div
          className={[
            css.bubble,
            isUser ? css.bubbleUser : css.bubbleAssistant,
            hasError ? css.bubbleError : "",
          ]
            .filter(Boolean)
            .join(" ")}
        >
          {hasError && message.error && (
            <ErrorBlock error={message.error} onRetry={onRetry} />
          )}
          {hasReasoning && (
            <ReasoningBlock
              content={message.reasoning!}
              isStreaming={isStreaming}
            />
          )}
          {hasToolCalls &&
            message.toolCalls!.map((tc) => (
              <ToolCallBlock key={tc.id} toolCall={tc} />
            ))}
          {message.content && (
            <div className={css.content}>
              {isUser ? (
                <p>{message.content}</p>
              ) : (
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {message.content}
                </ReactMarkdown>
              )}
            </div>
          )}
          {isStreaming && <span className={css.streamingCursor} />}
        </div>
        <div className={css.actions}>
          <button
            className={css.actionBtn}
            onClick={handleCopy}
            title="复制"
            type="button"
          >
            <CopyOutlined style={{ fontSize: 12 }} />
          </button>
          {onDelete && (
            <button
              className={css.actionBtn}
              onClick={handleDelete}
              title="删除"
              type="button"
            >
              <DeleteOutlined style={{ fontSize: 12 }} />
            </button>
          )}
        </div>
      </div>
      <div className={css.timestamp}>
        {new Date(message.timestamp).toLocaleTimeString()}
      </div>
    </div>
  );
};

export const RoleContext = React.createContext<string | undefined>(undefined);
