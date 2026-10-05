import React, { useCallback, useState } from "react";
import type { Message, ToolCall } from "../stores/chatStore";
import { ErrorBlock } from "./ErrorBlock";
import { ReasoningBlock } from "./ReasoningBlock";
import { ToolCallBlock } from "./ToolCallBlock";
import { EditBlock } from "./EditBlock";
import { MessageContent } from "./MessageContent";
import { MessageActions } from "./MessageActions";
import css from "./MessageBubble.module.css";

interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  compact?: boolean;
  onDelete?: (id: string) => void;
  onEdit?: (id: string, newContent: string) => void;
  onRegenerate?: (id: string) => void;
  onRetry?: () => void;
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({
  message,
  isStreaming = false,
  compact = false,
  onDelete,
  onEdit,
  onRegenerate,
  onRetry,
}) => {
  const isUser = message.role === "user";
  const hasError = message.error != null;
  const hasReasoning = !!message.reasoning;
  const hasToolCalls = !!message.toolCalls && message.toolCalls.length > 0;

  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(message.content);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content).catch(() => {});
  }, [message.content]);

  const handleDelete = useCallback(() => {
    onDelete?.(message.id);
  }, [onDelete, message.id]);

  const startEdit = useCallback(() => {
    setEditContent(message.content);
    setEditing(true);
  }, [message.content]);

  const confirmEdit = useCallback(() => {
    if (editContent.trim() && editContent !== message.content) {
      onEdit?.(message.id, editContent.trim());
    }
    setEditing(false);
  }, [editContent, message.id, message.content, onEdit]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setEditContent(message.content);
  }, [message.content]);

  const handleRegenerate = useCallback(() => {
    onRegenerate?.(message.id);
  }, [onRegenerate, message.id]);

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
            message.toolCalls!.map((tc: ToolCall) => (
              <ToolCallBlock key={tc.id} toolCall={tc} />
            ))}
          {editing ? (
            <EditBlock
              value={editContent}
              onChange={setEditContent}
              onConfirm={confirmEdit}
              onCancel={cancelEdit}
            />
          ) : (
            <>
              <MessageContent
                content={message.content}
                role={message.role}
                isStreaming={isStreaming}
              />
              {isStreaming && <span className={css.streamingCursor} />}
            </>
          )}
        </div>
        {!editing && (
          <div className={css.actionsWrapper}>
            <MessageActions
              role={message.role}
              isStreaming={isStreaming}
              onCopy={handleCopy}
              onDelete={onDelete ? handleDelete : undefined}
              onEdit={onEdit ? startEdit : undefined}
              onRegenerate={onRegenerate ? handleRegenerate : undefined}
            />
          </div>
        )}
      </div>
      <div className={css.timestamp}>
        {new Date(message.timestamp).toLocaleTimeString()}
      </div>
    </div>
  );
};

export const RoleContext = React.createContext<string | undefined>(undefined);
