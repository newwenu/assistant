import type { Message } from "./chatTypes";

/**
 * 乐观占位消息的唯一构造点。
 *
 * 占位消息尚未落库，但其字段必须与落库回读一致（I43 §4.1）：身份字段
 * （`modeId` / `providerId` / `modelId`）缺失会让流式首屏与回读结果视觉跳变。
 * 把「固定缺省值」收敛在这里，后续 wire 契约新增必填字段时只改这一处。
 */
export interface OptimisticMessageFields {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  timestamp: number;
  parentId: string | null;
  providerId?: string | null;
  modelId?: string | null;
  modeId?: string | null;
}

export function createOptimisticMessage(
  fields: OptimisticMessageFields,
): Message {
  return {
    id: fields.id,
    conversationId: fields.conversationId,
    role: fields.role,
    content: fields.content,
    tokens: null,
    timestamp: fields.timestamp,
    parentId: fields.parentId,
    isPinned: false,
    reasoning: null,
    siblingIndex: 0,
    siblingCount: null,
    providerId: fields.providerId ?? null,
    modelId: fields.modelId ?? null,
    modeId: fields.modeId ?? null,
  };
}
