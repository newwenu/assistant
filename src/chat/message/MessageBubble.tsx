import React, { useCallback, useState } from "react";
import { Typography } from "antd";
import { CompressOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { Message } from "../../chat/core/chatTypes";
import type { RetryNotification } from "../../chat/core/chatTypes";
import type {
  ImageBlock,
  FileBlock,
  ContentBlock,
} from "../core/contentBlockTypes";
import { ErrorBlock } from "../blocks/ErrorBlock";
import { RetryBanner } from "../blocks/RetryBanner";
import { getErrorPresentation } from "../core/chatErrorRecovery";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import { extractPlainText, parseContent } from "../core/contentBlockUtils";
import { translateKey } from "../../i18n";
import { ReasoningBlock } from "../blocks/ReasoningBlock";
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
  onErrorAction?: (code: RecoveryActionCode) => void;
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
    onErrorAction,
  }) => {
    const { t } = useTranslation();
    const isUser = message.role === "user";
    const isCompactSummary = message.role === "compact_summary";
    const hasError = message.error != null;
    const hasReasoning = !!message.reasoning;

    const [editing, setEditing] = useState(false);
    const [editContent, setEditContent] = useState(message.content);
    const [editImageBlocks, setEditImageBlocks] = useState<ImageBlock[]>([]);
    const [editFileBlocks, setEditFileBlocks] = useState<FileBlock[]>([]);

    const handleCopy = useCallback(() => {
      let text = message.content;
      if (message.error) {
        const presentation = getErrorPresentation(message.error);
        const errorParts: string[] = [];
        if (text) errorParts.push(text);
        errorParts.push(`[${translateKey(presentation.titleKey)}]`);
        if (message.error.message) errorParts.push(message.error.message);
        text = errorParts.join("\n");
      }
      navigator.clipboard.writeText(text).catch(() => {});
    }, [message.content, message.error]);

    const handleDelete = useCallback(() => {
      onDelete?.(message.id);
    }, [onDelete, message.id]);

    const startEdit = useCallback(() => {
      setEditContent(extractPlainText(message.content));
      const blocks = parseContent(message.content);
      const images = blocks
        ? blocks.filter((b): b is ImageBlock => b.type === "image")
        : [];
      const files = blocks
        ? blocks.filter((b): b is FileBlock => b.type === "file")
        : [];
      setEditImageBlocks(images);
      setEditFileBlocks(files);
      setEditing(true);
    }, [message.content]);

    const confirmEdit = useCallback(() => {
      const trimmed = editContent.trim();
      if (!trimmed) {
        setEditing(false);
        return;
      }

      const hasAttachments =
        editImageBlocks.length > 0 || editFileBlocks.length > 0;
      const originalBlocks = parseContent(message.content);
      const isMultimodal = hasAttachments || originalBlocks !== null;

      if (isMultimodal) {
        const newBlocks: ContentBlock[] = [
          { type: "text" as const, text: trimmed },
          ...editImageBlocks,
          ...editFileBlocks,
        ];
        const finalContent = JSON.stringify(newBlocks);
        onEdit?.(message.id, finalContent);
      } else {
        if (trimmed !== message.content) {
          onEdit?.(message.id, trimmed);
        }
      }
      setEditing(false);
    }, [
      editContent,
      editImageBlocks,
      editFileBlocks,
      message.id,
      message.content,
      onEdit,
    ]);

    const cancelEdit = useCallback(() => {
      setEditing(false);
      setEditContent(extractPlainText(message.content));
      setEditImageBlocks([]);
      setEditFileBlocks([]);
    }, [message.content]);

    const handleRemoveEditImage = useCallback((index: number) => {
      setEditImageBlocks((prev) => prev.filter((_, i) => i !== index));
    }, []);

    const handleRemoveEditFile = useCallback((index: number) => {
      setEditFileBlocks((prev) => prev.filter((_, i) => i !== index));
    }, []);

    const handleRegenerate = useCallback(() => {
      onRegenerate?.(message.id);
    }, [onRegenerate, message.id]);

    const prevRole = React.useContext(RoleContext);
    const isGroupGap = prevRole !== undefined && prevRole !== message.role;

    const errorSeverity =
      hasError && message.error
        ? getErrorPresentation(message.error).severity
        : null;

    const rootClass = [
      css.root,
      isUser
        ? css.rootRoleUser
        : isCompactSummary
          ? css.rootRoleCompactSummary
          : css.rootRoleAssistant,
      isGroupGap ? css.rootGroupGap : "",
      compact ? css.compact : "",
      editing ? css.rootEditing : "",
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
              errorSeverity === "warning"
                ? css.bubbleWarning
                : hasError
                  ? css.bubbleError
                  : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            {hasError && message.error && (
              <ErrorBlock
                error={message.error}
                onRetry={onRetry}
                onAction={onErrorAction}
              />
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
            {editing ? (
              <EditBlock
                value={editContent}
                onChange={setEditContent}
                onConfirm={confirmEdit}
                onCancel={cancelEdit}
                imageBlocks={editImageBlocks}
                fileBlocks={editFileBlocks}
                onRemoveImage={handleRemoveEditImage}
                onRemoveFile={handleRemoveEditFile}
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
    return true;
  },
);

export const RoleContext = React.createContext<string | undefined>(undefined);
