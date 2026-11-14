import React, { useState } from "react";
import type { Message, RetryNotification } from "../core/chatTypes";
import type { RecoveryActionCode } from "../core/chatErrorRecovery";
import { GroupHeader } from "./GroupHeader";
import { IdentityNode } from "./IdentityNode";
import { MessageBubble, RoleContext } from "./MessageBubble";
import { ReasoningNode } from "./ReasoningNode";
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

  // 组头只在 run 的首个节点前渲染。run 的节点之间可能夹着同一消息的
  // 身份行 / `chromeOnly` 气泡壳（无正文的 assistant 行），它们不承载 run，
  // 不能把「上一次见到的 run」重置为 `null`——否则第一个工具节点会再次判定成
  // run 起点，组头在同一段里渲染两遍（「每个节点都显示 M 思考」）。
  let lastRunId: string | null = null;
  const nodes = group.nodes.map((node) => {
    if (node.kind === "reasoning") {
      const run = node.run;
      const isRunStart = run !== null && lastRunId !== run.id;
      if (run !== null) lastRunId = run.id;
      const expanded = run !== null && expandedRuns[run.id] === true;
      const density = run === null ? "block" : expanded ? "expanded" : "single";
      return (
        <React.Fragment key={`reasoning:${node.messageId}`}>
          {isRunStart && run !== null && (
            <GroupHeader
              steps={run.steps}
              reasoning={run.reasoning}
              failed={run.failed}
              durationMs={run.durationMs}
              expanded={expanded}
              onToggle={() => toggleRun(run.id)}
            />
          )}
          <ReasoningNode
            text={node.text}
            density={density}
            isStreaming={
              isStreaming &&
              node.messageId === streamingMessageId &&
              isReasoningStreaming
            }
          />
        </React.Fragment>
      );
    }

    if (node.kind === "tool") {
      const run = node.run;
      const isRunStart = run !== null && lastRunId !== run.id;
      if (run !== null) lastRunId = run.id;
      const expanded = run !== null && expandedRuns[run.id] === true;
      return (
        <React.Fragment key={`tool:${node.callId}`}>
          {isRunStart && run !== null && (
            <GroupHeader
              steps={run.steps}
              reasoning={run.reasoning}
              failed={run.failed}
              durationMs={run.durationMs}
              expanded={expanded}
              onToggle={() => toggleRun(run.id)}
            />
          )}
          <ToolCallRenderer
            view={node.view}
            density={run !== null && !expanded ? "single" : "card"}
            docPanelPath={docPanelPath}
            onOpenDocument={onOpenDocument}
          />
        </React.Fragment>
      );
    }

    if (node.kind === "identity") {
      return (
        <IdentityNode
          key={`identity:${node.message.id}`}
          message={node.message}
          compact={compact}
        />
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
          chromeOnly={node.chromeOnly}
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
