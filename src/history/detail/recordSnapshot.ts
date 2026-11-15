import type { Message, ToolCallView } from "../../chat/core/chatTypes";
import type { MessageRecordDto } from "../../types/protocol.contract";

/**
 * 原始视图的数据来源（I51 §6.3）：
 * - `"dto"`：前端 `Message` 投影出的快照，携带前端补齐的字段（如 `toolCalls` 已解析）；
 * - `"raw"`：`get_conversation` 返回的落库原文行（`tool_calls_json` 仍是原串）。
 *
 * 两者形近但不等价，所以块头必须标出来源，避免使用者把快照当成落库事实。
 */
export type RecordSource = "dto" | "raw";

/**
 * 原始视图块正文：有落库原文就用原文，没有则退回前端快照。
 *
 * 退回是刻意的降级路径：拉取失败时不能留空白块，使用者至少还能看到快照。
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
