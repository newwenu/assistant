import type { Message } from "@/chat/core/chatTypes";
import type { ConversationRoute } from "@/chat/core/message/conversationLink";
import type { MessageDto } from "@/types/protocol.contract";

/**
 * 消息夹具的唯一构造点。
 *
 * `Message` / `MessageDto` 的必填字段随 wire 契约增长（如 I43 的 `modeId`），
 * 若各测试各写一份完整字面量，每次加字段都要全仓库改一遍。因此：
 * 新增必填字段时**只改这里**的基线对象，各测试通过 `overrides` 表达差异。
 */
const messageBase: Omit<Message, "id"> = {
  conversationId: "conv-1",
  role: "user",
  content: "",
  tokens: null,
  timestamp: 0,
  parentId: null,
  isPinned: false,
  reasoning: null,
  siblingIndex: 0,
  siblingCount: null,
  providerId: null,
  modelId: null,
  modeId: null,
  turnIndex: null,
  iteration: 0,
};

export function makeMessage(
  id: string,
  overrides: Partial<Omit<Message, "id">> = {},
): Message {
  return { ...messageBase, id, ...overrides };
}

/** wire 形态夹具：比 `Message` 多 `status` / `toolCalls` 两个键。 */
const messageDtoBase: Omit<MessageDto, "id"> = {
  ...messageBase,
  status: "completed",
  toolCalls: [],
};

export function makeMessageDto(
  id: string,
  overrides: Partial<Omit<MessageDto, "id">> = {},
): MessageDto {
  return { ...messageDtoBase, id, ...overrides };
}

/** 链路判定夹具；缺省为 chat 链路（`modeId` / `toolConfig` 皆空）。 */
export function makeConversationRoute(
  overrides: Partial<ConversationRoute> = {},
): ConversationRoute {
  return { link: "chat", modeId: null, toolConfig: null, ...overrides };
}
