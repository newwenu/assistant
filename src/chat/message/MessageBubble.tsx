import React, { useCallback, useState } from "react";
import { Typography } from "antd";
import { CompressOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { Message, ToolCall } from "../../stores/chatStore";
import type { RetryNotification } from "../../chat/core/chatTypes";
import { ErrorBlock } from "../blocks/ErrorBlock";
import { RetryBanner } from "../blocks/RetryBanner";
import { getErrorDisplay, ChatErrorCode } from "../core/chatErrors";
import { ReasoningBlock } from "../blocks/ReasoningBlock";
import { ToolCallBlock } from "../blocks/ToolCallBlock";
import { EditBlock } from "../blocks/EditBlock";
import { BranchPicker } from "../blocks/BranchPicker";
import { MessageContent } from "./MessageContent";
import { MessageActions } from "./MessageActions";
import css from "./MessageBubble.module.css";

interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  isReasoningStreaming?: boolean;
  retryStatus?: RetryNotification | null;
  compact?: boolean;
  onDelete?: (id: string) => void;
  onEdit?: (id: string, newContent: string) => void;
  onRegenerate?: (id: string) => void;
  onRetry?: () => void;
  onSwitchBranch?: (messageId: string, direction: "prev" | "next") => void;
  onFork?: (messageId: string) => void;
}

export const MessageBubble: React.FC<MessageBubbleProps> = React.memo(
  ({
    message,
    isStreaming = false,
    isReasoningStreaming = false,
    retryStatus = null,
    compact = false,
    onDelete,
    onEdit,
    onRegenerate,
    onRetry,
    onSwitchBranch,
    onFork,
  }) => {
    const { t } = useTranslation();
    const isUser = message.role === "user";
    const isCompactSummary = message.role === "compact_summary";
    const hasError = message.error != null;
    const hasReasoning = !!message.reasoning;
    const hasToolCalls = !!message.toolCalls && message.toolCalls.length > 0;

    const [editing, setEditing] = useState(false);
    const [editContent, setEditContent] = useState(message.content);

    const handleCopy = useCallback(() => {
      let text = message.content;
      if (message.error) {
        const display =
          getErrorDisplay(message.error.code) ??
          getErrorDisplay(ChatErrorCode.InternalError);
        const errorParts: string[] = [];
        if (text) errorParts.push(text);
        errorParts.push(`[${display.title}]`);
        if (message.error.message) errorParts.push(message.error.message);
        text = errorParts.join("\n");
      }
      navigator.clipboard.writeText(text).catch(() => {});
    }, [message.content, message.error]);

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
      isUser
        ? css.rootRoleUser
        : isCompactSummary
          ? css.rootRoleCompactSummary
          : css.rootRoleAssistant,
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
            {retryStatus && isStreaming && (
              <RetryBanner
                retryStatus={retryStatus}
                onCancel={() => onRetry?.()}
              />
            )}
            {isCompactSummary && (
              <div className={css.compactSummaryHeader}>
                <CompressOutlined style={{ marginRight: 6 }} />
                <Typography.Text type="warning" strong>
                  {t("chat.compact.summary_label")}
                </Typography.Text>
              </div>
            )}
            {hasReasoning && (
              <ReasoningBlock
                content={message.reasoning!}
                isStreaming={isReasoningStreaming}
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
                {isStreaming && !isReasoningStreaming && (
                  <span className={css.streamingCursor} />
                )}
              </>
            )}
          </div>
          {!editing && (
            <div className={css.actionsWrapper}>
              <BranchPicker
                currentSiblingIndex={message.siblingIndex}
                siblingCount={message.siblingCount ?? 1}
                onSwitch={(direction) => {
                  onSwitchBranch?.(message.id, direction);
                }}
                onFork={onFork ? () => onFork(message.id) : undefined}
              />
              <MessageActions
                role={message.role}
                isStreaming={isStreaming}
                isBranchAtLimit={(message.siblingCount ?? 1) >= 20}
                onCopy={handleCopy}
                onDelete={onDelete ? handleDelete : undefined}
                onEdit={onEdit ? startEdit : undefined}
                onRegenerate={onRegenerate ? handleRegenerate : undefined}
                onFork={onFork ? () => onFork(message.id) : undefined}
              />
            </div>
          )}
        </div>
        <div className={css.timestamp}>
          {new Date(message.timestamp).toLocaleTimeString()}
        </div>
      </div>
    );
  },
  (prev, next) => {
    if (prev.message.id !== next.message.id) return false;
    if (prev.message.content !== next.message.content) return false;
    if (prev.message.reasoning !== next.message.reasoning) return false;
    if (prev.message.error !== next.message.error) return false;
    if (prev.message.timestamp !== next.message.timestamp) return false;
    if (prev.isStreaming !== next.isStreaming) return false;
    if (prev.isReasoningStreaming !== next.isReasoningStreaming) return false;
    if (prev.retryStatus !== next.retryStatus) return false;
    if (prev.compact !== next.compact) return false;
    if (prev.onDelete !== next.onDelete) return false;
    if (prev.onEdit !== next.onEdit) return false;
    if (prev.onRegenerate !== next.onRegenerate) return false;
    if (prev.onRetry !== next.onRetry) return false;
    if (prev.onSwitchBranch !== next.onSwitchBranch) return false;
    if (prev.onFork !== next.onFork) return false;
    if (prev.message.siblingIndex !== next.message.siblingIndex) return false;
    if (prev.message.siblingCount !== next.message.siblingCount) return false;
    const prevTc = prev.message.toolCalls;
    const nextTc = next.message.toolCalls;
    if (prevTc !== nextTc) {
      if (!prevTc || !nextTc || prevTc.length !== nextTc.length) return false;
      for (let i = 0; i < prevTc.length; i++) {
        if (
          prevTc[i].id !== nextTc[i].id ||
          prevTc[i].name !== nextTc[i].name ||
          prevTc[i].arguments !== nextTc[i].arguments
        )
          return false;
      }
    }
    return true;
  },
);

export const RoleContext = React.createContext<string | undefined>(undefined);
