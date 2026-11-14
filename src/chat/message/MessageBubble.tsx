import React, { useCallback, useMemo, useState } from "react";
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
import { ReasoningBlock } from "../blocks/ReasoningBlock";
import { ToolCallRenderer } from "../tool/ToolCallRenderer";
import { EditBlock } from "../blocks/EditBlock";
import { BranchPicker } from "../blocks/BranchPicker";
import { MessageContent } from "./MessageContent";
import { MessageActions } from "./MessageActions";
import { MessageAvatar } from "./MessageAvatar";
import { TurnTail, buildCopyText } from "./TurnTail";
import { resolveMessageIdentity } from "./messageIdentity";
import { useAgentModeStore } from "../../stores/agentModeStore";
import { useProviderStore } from "../../stores/providerStore";
import css from "./MessageBubble.module.css";

interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  isReasoningStreaming?: boolean;
  retryStatus?: RetryNotification | null;
  compact?: boolean;
  /**
   * 是否渲染头像与身份行（I43 §4.4）：`MessageList` 按轮次起点显式传入，
   * 历史树详情等无列表上下文的场景走缺省 `true`。
   */
  showIdentity?: boolean;
  showBranchPicker?: boolean;
  /** true 时 assistant 侧 chrome 由 `TurnTail` 承接，本气泡不渲染（I46 §4.4）。 */
  suppressAssistantChrome?: boolean;
  /** 本行是所在 turn 的尾锚点：由它挂载唯一的 `TurnTail`。 */
  isTurnTail?: boolean;
  onDelete?: (id: string) => void;
  onEdit?: (id: string, newContent: string) => void;
  onRegenerate?: (id: string) => void;
  onRetry?: () => void;
  onSwitchBranch?: (messageId: string, direction: "prev" | "next") => void;
  onFork?: (messageId: string) => void;
  onErrorAction?: (code: RecoveryActionCode) => void;
  /** 文档面板当前打开的文件路径；`null` 时工具结果恒走卡片（I46 compact §3.7.4）。 */
  docPanelPath?: string | null;
  onOpenDocument?: (path: string, line: number) => void;
}

export const MessageBubble: React.FC<MessageBubbleProps> = React.memo(
  ({
    message,
    isStreaming = false,
    isReasoningStreaming = false,
    retryStatus = null,
    compact = false,
    showIdentity = true,
    showBranchPicker = true,
    suppressAssistantChrome = false,
    isTurnTail = false,
    onDelete,
    onEdit,
    onRegenerate,
    onRetry,
    onSwitchBranch,
    onFork,
    onErrorAction,
    docPanelPath = null,
    onOpenDocument,
  }) => {
    const { t } = useTranslation();
    const isUser = message.role === "user";
    const isCompactSummary = message.role === "compact_summary";
    /** 身份指示只对 `role === "assistant"` 生效（I43 N1），`tool` 等角色不参与。 */
    const isAssistant = message.role === "assistant";
    const identityVisible = isAssistant && showIdentity;
    const hasError = message.error != null;
    const hasReasoning = !!message.reasoning;

    const modes = useAgentModeStore((s) => s.modes);
    const providers = useProviderStore((s) => s.providers);
    const identity = useMemo(
      () =>
        identityVisible
          ? resolveMessageIdentity(message, modes, providers)
          : null,
      [identityVisible, message, modes, providers],
    );

    const [editing, setEditing] = useState(false);
    const [editContent, setEditContent] = useState(message.content);
    const [editImageBlocks, setEditImageBlocks] = useState<ImageBlock[]>([]);
    const [editFileBlocks, setEditFileBlocks] = useState<FileBlock[]>([]);

    const handleCopy = useCallback(() => {
      navigator.clipboard.writeText(buildCopyText(message)).catch(() => {});
    }, [message]);

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

    const hasBody =
      message.content.trim().length > 0 ||
      hasReasoning ||
      (message.toolCalls?.length ?? 0) > 0 ||
      isCompactSummary ||
      editing;
    const showBubble = hasBody || (!hasError && !retryStatus);

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

    const identityModeText = identity
      ? (identity.modeName ?? t("chat.message.identity.unknown_mode"))
      : null;
    const identityTitle = identity
      ? [identityModeText, identity.modelName].filter(Boolean).join(" ")
      : undefined;

    return (
      <div className={rootClass} data-role={message.role}>
        {identity && identityModeText !== null && (
          <div className={css.identityRow} title={identityTitle}>
            <MessageAvatar modeId={message.modeId} />
            <span className={css.identityMode}>{identityModeText}</span>
            {identity.modelName && (
              <>
                <span className={css.identitySeparator}>
                  {t("chat.message.identity.separator")}
                </span>
                <span className={css.identityModel}>{identity.modelName}</span>
              </>
            )}
          </div>
        )}
        <div className={css.stack}>
          {showBubble && (
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
              {message.toolCalls && message.toolCalls.length > 0 && (
                <div className={css.toolCalls}>
                  {message.toolCalls.map((view) => (
                    <ToolCallRenderer
                      key={view.callId}
                      view={view}
                      density="card"
                      docPanelPath={docPanelPath}
                      onOpenDocument={onOpenDocument}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
          {!isStreaming && message.status === "interrupted" && (
            <div className={css.interruptedNote}>
              <Typography.Text type="secondary">
                {t("chat.message.interrupted")}
              </Typography.Text>
            </div>
          )}
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
          {!editing && !(suppressAssistantChrome && isAssistant) && (
            <div className={css.actionsWrapper}>
              <span className={css.timestamp}>
                {new Date(message.timestamp).toLocaleTimeString()}
              </span>
              {showBranchPicker && (
                <BranchPicker
                  currentSiblingIndex={message.siblingIndex}
                  siblingCount={message.siblingCount ?? 1}
                  onSwitch={(direction) => {
                    onSwitchBranch?.(message.id, direction);
                  }}
                  onFork={onFork ? () => onFork(message.id) : undefined}
                />
              )}
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
          {!editing && suppressAssistantChrome && isAssistant && isTurnTail && (
            <TurnTail
              anchor={message}
              isStreaming={isStreaming}
              isBranchAtLimit={(message.siblingCount ?? 1) >= 20}
              onDelete={onDelete}
              onRegenerate={onRegenerate}
              onSwitchBranch={onSwitchBranch}
              onFork={onFork}
              onCopy={(text) => {
                navigator.clipboard.writeText(text).catch(() => {});
              }}
            />
          )}
        </div>
      </div>
    );
  },
  (prev, next) => {
    if (prev.message.id !== next.message.id) return false;
    if (prev.message.content !== next.message.content) return false;
    if (prev.message.reasoning !== next.message.reasoning) return false;
    if (prev.message.toolCalls !== next.message.toolCalls) return false;
    if (prev.message.error !== next.message.error) return false;
    if (prev.message.timestamp !== next.message.timestamp) return false;
    if (prev.isStreaming !== next.isStreaming) return false;
    if (prev.isReasoningStreaming !== next.isReasoningStreaming) return false;
    if (prev.retryStatus !== next.retryStatus) return false;
    if (prev.compact !== next.compact) return false;
    if (prev.showBranchPicker !== next.showBranchPicker) return false;
    if (prev.onDelete !== next.onDelete) return false;
    if (prev.onEdit !== next.onEdit) return false;
    if (prev.onRegenerate !== next.onRegenerate) return false;
    if (prev.onRetry !== next.onRetry) return false;
    if (prev.onSwitchBranch !== next.onSwitchBranch) return false;
    if (prev.onFork !== next.onFork) return false;
    if (prev.message.siblingIndex !== next.message.siblingIndex) return false;
    if (prev.message.siblingCount !== next.message.siblingCount) return false;
    if (prev.message.modeId !== next.message.modeId) return false;
    if (prev.message.modelId !== next.message.modelId) return false;
    if (prev.message.providerId !== next.message.providerId) return false;
    if (prev.showIdentity !== next.showIdentity) return false;
    if (prev.suppressAssistantChrome !== next.suppressAssistantChrome)
      return false;
    if (prev.isTurnTail !== next.isTurnTail) return false;
    if (prev.message.turnIndex !== next.message.turnIndex) return false;
    if (prev.message.iteration !== next.message.iteration) return false;
    if (prev.message.status !== next.message.status) return false;
    return true;
  },
);

export const RoleContext = React.createContext<string | undefined>(undefined);
