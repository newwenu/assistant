import { useChatStreamStore } from "../stores/chat/chatStreamStore";
import { useChatStore } from "../stores/chat/chatStore";
import { hasPendingToolCalls } from "../chat/core/toolCallView";
import type { Message } from "../chat/core/chatTypes";

/**
 * Orb 运行态判定（I47 §4.2）：正在生成，或存在未收敛的工具调用。
 *
 * `hasPendingToolCalls` 已覆盖 `running` 与 `awaiting_user`，而等待审批与等待回答
 * 都收敛为 `awaiting_user`，因此单一判定即可覆盖全部等待态，不需要第二个信号。
 */
export function orbBusyFrom(
  isStreaming: boolean,
  messages: Message[],
): boolean {
  if (isStreaming) return true;
  return hasPendingToolCalls(messages.flatMap((m) => m.toolCalls ?? []));
}

/** 供事件回调取值；不参与渲染，故直接读 store 快照而不订阅。 */
export function isOrbBusy(): boolean {
  return orbBusyFrom(
    useChatStreamStore.getState().isStreaming,
    useChatStore.getState().messages,
  );
}
