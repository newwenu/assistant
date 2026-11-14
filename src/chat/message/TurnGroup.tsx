import React, { useState } from "react";
import type { Message, RetryNotification } from "../core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import { GroupHeader } from "./GroupHeader";
import { MessageBubble, RoleContext } from "./MessageBubble";
import { ToolCallRenderer } from "../tool/ToolCallRenderer";
import type { NodeGroupView } from "./groupNodeRows";
import css from "./TurnGroup.module.css";

interface TurnGroupHandlers {
  onDelete?: (id: string) => void;
  onEdit?: (id: string, newContent: string) => void;
  onRegenerate?: (id: string) => void;
  onRetryMessage?: (messageId: string) => void;
  onSwitchBranch?: (messageId: string, direction: "prev" | "next") => void;
  onFork?: (messageId: string) => void;
  onErrorAction?: (code: RecoveryActionCode) => void;
}

interface TurnGroupProps {
  group: NodeGroupView;
  handlers: TurnGroupHandlers;
  /** 上一条消息的角色（按全局顺序），供 `RoleContext` 间距判定。 */
  prevRoleById: Map<string, string | undefined>;
  isStreaming?: boolean;
  isReasoningStreaming?: boolean;
  streamingMessageId?: string | null;
  streamingContent?: string | null;
  retryStatus?: RetryNotification | null;
  compact?: boolean;
  /** 列表中最后一条 assistant 消息 id：只有它允许重新生成。 */
  lastAssistantId?: string | null;
  /** 文档面板当前打开的文件路径；`null` 时工具结果恒走卡片（I46 compact §3.7.4）。 */
  docPanelPath?: string | null;
  onOpenDocument?: (path: string, line: number) => void;
}

/**
 * 一个 turn 的挂载单元（I46 §5.5）：
 * `[身份行] + [回复行 / 过程组（组头 + 步行）] + [TurnTail]`。
 *
 * 展开态是本地 state（ADR D6），**按 run 各自持有**：一个 turn 内可以有多个
 * 独立折叠段（被失败 / 待处理 / 无摘要的步切断），每段自己的组头、自己的
 * 计数、自己的展开开关，互不污染。`turnIndex` 变化时由父级 key 重建。
 */
export const TurnGroup: React.FC<TurnGroupProps> = ({
  group,
  handlers,
  prevRoleById,
  isStreaming = false,
  isReasoningStreaming = false,
  streamingMessageId = null,
  streamingContent = null,
  retryStatus = null,
  compact = false,
  lastAssistantId = null,
  docPanelPath = null,
  onOpenDocument,
}) => {
  const [expandedRuns, setExpandedRuns] = useState<Record<string, boolean>>({});

  const toggleRun = (runId: string) =>
    setExpandedRuns((prev) => ({ ...prev, [runId]: !prev[runId] }));

  const nodes = group.nodes.map((node, index) => {
    if (node.kind === "tool") {
      const prev = index > 0 ? group.nodes[index - 1] : undefined;
      const isRunStart =
        prev === undefined ||
        prev.kind !== "tool" ||
        prev.run.id !== node.run.id;
      const expanded = expandedRuns[node.run.id] === true;
      return (
        <React.Fragment key={`tool:${node.callId}`}>
          {isRunStart && (
            <GroupHeader
              steps={node.run.steps}
              failed={node.run.failed}
              durationMs={node.run.durationMs}
              expanded={expanded}
              onToggle={() => toggleRun(node.run.id)}
            />
          )}
          <ToolCallRenderer
            view={node.view}
            density={expanded ? "card" : "single"}
            docPanelPath={docPanelPath}
            onOpenDocument={onOpenDocument}
          />
        </React.Fragment>
      );
    }

    const message = node.message;
    const isStreamingMessage = isStreaming && message.id === streamingMessageId;
    const displayMessage: Message =
      isStreamingMessage && streamingContent != null
        ? { ...message, content: streamingContent }
        : message;
    const anchor = group.tailAnchor;
    return (
      <RoleContext.Provider
        key={message.id}
        value={prevRoleById.get(message.id)}
      >
        <MessageBubble
          message={displayMessage}
          isStreaming={isStreamingMessage}
          isReasoningStreaming={isStreamingMessage && isReasoningStreaming}
          retryStatus={isStreamingMessage ? retryStatus : null}
          compact={compact}
          showIdentity={node.showIdentity}
          suppressAssistantChrome={!node.ownsAssistantChrome}
          isTurnTail={anchor !== null && anchor.id === message.id}
          onDelete={handlers.onDelete}
          onEdit={handlers.onEdit}
          onRegenerate={
            message.id === lastAssistantId ? handlers.onRegenerate : undefined
          }
          onRetry={
            message.error?.retryable
              ? () => handlers.onRetryMessage?.(message.id)
              : undefined
          }
          onSwitchBranch={handlers.onSwitchBranch}
          onFork={handlers.onFork}
          onErrorAction={handlers.onErrorAction}
          docPanelPath={docPanelPath}
          onOpenDocument={onOpenDocument}
        />
      </RoleContext.Provider>
    );
  });

  return <div className={css.root}>{nodes}</div>;
};
