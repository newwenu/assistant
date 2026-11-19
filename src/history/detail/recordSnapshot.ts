import type { Message, ToolCallView } from "../../chat/core/chatTypes";
import type { MessageRecordDto } from "../../types/protocol.contract";

/**
 * 记录原文面板的数据来源（I51 §4.5）：
 * - `"loading"`：拉取在途，面板先展示前端快照，但标签必须标明「加载中」；
 * - `"raw"`：`get_conversation` 返回的落库原文行（`tool_calls_json` 仍是原串）；
 * - `"dto"`：前端 `Message` 投影出的快照，同时承担「该 id 无落库记录」与「拉取失败」两种降级。
 *
 * 三态不可合并：加载态与降级态外观相同会把快照误当落库事实。
 */
export type RecordSource = "loading" | "raw" | "dto";

/**
 * 行内原文面板的状态入参（I51 §4.5）。
 *
 * 禁止用 `record == null` 反推来源：加载中同样没有 `record`。
 */
export interface HistoryRecordState {
  source: RecordSource;
  record: MessageRecordDto | null;
  error: string | null;
}

/**
 * 记录原文面板正文：有落库原文就用原文，没有则退回前端快照。
 *
 * 退回是刻意的降级路径：拉取失败时不能留空白面板，使用者至少还能看到快照。
 */
export function toRawRecordJson(
  record: MessageRecordDto | null,
  message: Message,
): string {
  return JSON.stringify(record ?? toRecordSnapshot(message), null, 2);
}

function toToolCallRecord(view: ToolCallView): Record<string, unknown> {
  return {
    callId: view.callId,
    name: view.name,
    arguments: view.arguments ?? null,
    status: view.status,
    result: view.resultText ?? null,
    compact: view.compact ?? null,
  };
}

/**
 * 落库记录快照（I51 §4.3）：把前端 `Message` 投影为与 `MessageDto` 同形的只读记录。
 *
 * 禁止直接序列化 `Message`：它携带前端本地字段（`error` / `argsText` /
 * `argsPreview` / `startedAt` 等），直接输出会把运行期状态混进「落库记录」。
 * 缺值一律输出 `null`（键恒在），禁止 `undefined`——`JSON.stringify` 会整键丢弃。
 */
export function toRecordSnapshot(message: Message): Record<string, unknown> {
  return {
    id: message.id,
    conversationId: message.conversationId,
    role: message.role,
    content: message.content,
    tokens: message.tokens ?? null,
    timestamp: message.timestamp,
    parentId: message.parentId ?? null,
    isPinned: message.isPinned,
    reasoning: message.reasoning ?? null,
    siblingIndex: message.siblingIndex,
    siblingCount: message.siblingCount ?? null,
    providerId: message.providerId ?? null,
    modelId: message.modelId ?? null,
    modeId: message.modeId ?? null,
    status: message.status ?? null,
    toolCalls: (message.toolCalls ?? []).map(toToolCallRecord),
    turnIndex: message.turnIndex ?? null,
    iteration: message.iteration,
  };
}
