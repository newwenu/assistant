import { translateKey } from "../i18n";
import type { Conversation } from "../chat/core/chatTypes";

export const COPY_TITLE_SOURCE = "copy";

export function formatConversationTitle(
  conversation: Pick<Conversation, "title" | "titleSource">,
): string {
  if (!conversation.title || conversation.titleSource !== COPY_TITLE_SOURCE) {
    return conversation.title;
  }
  return `${conversation.title}${translateKey("history.copy_suffix")}`;
}
