import React from "react";
import type { Message } from "../../chat/core/chatTypes";
import { getErrorPresentation } from "../core/chatErrorRecovery";
import { translateKey } from "../../i18n";
import { BranchPicker } from "../blocks/BranchPicker";
import { MessageActions } from "./MessageActions";
import css from "./MessageBubble.module.css";

/** 复制文本的唯一构造点（I46 §4.2）：失败消息把错误标题与详情一并带上。 */
export function buildCopyText(message: Message): string {
  let text = message.content;
  if (message.error) {
    const presentation = getErrorPresentation(message.error);
    const parts: string[] = [];
    if (text) parts.push(text);
    parts.push(`[${translateKey(presentation.titleKey)}]`);
    if (message.error.message) parts.push(message.error.message);
    text = parts.join("\n");
  }
  return text;
}

interface TurnTailProps {
  /** 锚点行（assistant 侧）；调用方保证非空。 */
  anchor: Message;
  isStreaming?: boolean;
  isBranchAtLimit?: boolean;
  onDelete?: (id: string) => void;
  onRegenerate?: (id: string) => void;
  onSwitchBranch?: (id: string, direction: "prev" | "next") => void;
  onFork?: (id: string) => void;
  onCopy?: (text: string) => void;
}

/**
 * turn 尾 chrome（I46 §4.1 / §4.2）：时间戳 + `BranchPicker` + `MessageActions`
 * 每轮只挂一次。只读锚点行的既有字段，不新增消息行、不改分支链；
 * 样式复用 `MessageBubble.module.css`，挂载位置与该气泡现状一致。
 */
export const TurnTail: React.FC<TurnTailProps> = ({
  anchor,
  isStreaming = false,
  isBranchAtLimit = false,
  onDelete,
  onRegenerate,
  onSwitchBranch,
  onFork,
  onCopy,
}) => {
  const siblingCount = anchor.siblingCount ?? 1;
  return (
    <div className={css.actionsWrapper}>
      <span className={css.timestamp}>
        {new Date(anchor.timestamp).toLocaleTimeString()}
      </span>
      <BranchPicker
        currentSiblingIndex={anchor.siblingIndex}
        siblingCount={siblingCount}
        onSwitch={(direction) => {
          onSwitchBranch?.(anchor.id, direction);
        }}
        onFork={onFork ? () => onFork(anchor.id) : undefined}
      />
      <MessageActions
        role="assistant"
        isStreaming={isStreaming}
        isBranchAtLimit={isBranchAtLimit || siblingCount >= 20}
        onCopy={onCopy ? () => onCopy(buildCopyText(anchor)) : undefined}
        onDelete={onDelete ? () => onDelete(anchor.id) : undefined}
        onRegenerate={onRegenerate ? () => onRegenerate(anchor.id) : undefined}
        onFork={onFork ? () => onFork(anchor.id) : undefined}
      />
    </div>
  );
};
