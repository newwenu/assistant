import { extractDerivedText } from "../../chat/core/message/contentBlockUtils";
import { CHAT_AUTO_TITLE_LENGTH } from "../../config/constants";

/** 无自动标题时用首条用户消息兜底：取首个非空行，按标点或长度截断。 */
export function fallbackTitleFromUserMessage(
  userMessage: string,
  maxLength = CHAT_AUTO_TITLE_LENGTH,
): string {
  const plainText = extractDerivedText(userMessage);
  const firstLine = plainText.split("\n").find((l) => l.trim()) || "";
  if (firstLine.length <= maxLength) return firstLine.trim();
  const puncts = ["。", "？", "！", ".", "?", "!", "；", ";", "，", ","];
  for (const p of puncts) {
    const idx = firstLine.indexOf(p);
    if (idx > 0 && idx < maxLength) {
      return firstLine.slice(0, idx + 1).trim();
    }
  }
  return firstLine.slice(0, maxLength).trim() + "…";
}
