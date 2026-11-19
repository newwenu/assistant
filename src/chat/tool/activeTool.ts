import type { Message, ToolCallView } from "../core/chatTypes";

/**
 * 取「当前活动的工具」：优先最后一个 `running`，否则最后一个 `preparing`；都没有则 `null`。
 *
 * 参数生成必然早于任何执行，一旦出现 `Started` 就说明准备已结束，
 * 因此 `running` 优先于位置顺序，避免指示器停在「准备中」而实际已在执行。
 */
export function pickActiveTool(messages: Message[]): ToolCallView | null {
  let running: ToolCallView | null = null;
  let preparing: ToolCallView | null = null;
  for (const message of messages) {
    if (message.role !== "assistant" || !message.toolCalls?.length) continue;
    for (const view of message.toolCalls) {
      if (view.status === "running") running = view;
      else if (view.status === "preparing") preparing = view;
    }
  }
  return running ?? preparing;
}
