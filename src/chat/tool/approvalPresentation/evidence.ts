import {
  COMMAND_WIDTH_LIMIT,
  CONTENT_MAX_BYTES,
  CONTENT_MAX_LINES,
  DIFF_MAX_LINES,
} from "./tables";
import {
  displayWidth,
  hasControlOperator,
  splitLines,
  utf8Length,
} from "./text";
import type { ApprovalEvidence, TranslateFn } from "./types";

/** §5.3 证据形态：由参数形态决定，不按 `toolName` 选块。 */

/** 命令是否进恒展开的命令块：超宽或含控制操作符（§5.3 / R1）。 */
export function needsCommandBlock(command: string): boolean {
  return (
    hasControlOperator(command) || displayWidth(command) > COMMAND_WIDTH_LIMIT
  );
}

export function diffEvidence(
  oldString: string,
  newString: string,
  t: TranslateFn,
): ApprovalEvidence {
  const oldLines = splitLines(oldString);
  const newLines = splitLines(newString);
  const rendered: string[] = [];
  for (const line of oldLines.slice(0, DIFF_MAX_LINES))
    rendered.push(`- ${line}`);
  for (const line of newLines.slice(0, DIFF_MAX_LINES))
    rendered.push(`+ ${line}`);
  const omitted =
    Math.max(0, oldLines.length - DIFF_MAX_LINES) +
    Math.max(0, newLines.length - DIFF_MAX_LINES);
  if (omitted > 0) {
    rendered.push(t("chat.tool.approval.evidence.omitted", { count: omitted }));
  }
  return {
    kind: "diff",
    text: rendered.join("\n"),
    truncated: omitted > 0,
  };
}

export function contentEvidence(
  content: string,
  t: TranslateFn,
): ApprovalEvidence {
  const lines = splitLines(content);
  let text = lines.slice(0, CONTENT_MAX_LINES).join("\n");
  let truncated = lines.length > CONTENT_MAX_LINES;
  if (utf8Length(text) > CONTENT_MAX_BYTES) {
    let low = 0;
    let high = text.length;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if (utf8Length(text.slice(0, middle)) <= CONTENT_MAX_BYTES) {
        low = middle;
      } else {
        high = middle - 1;
      }
    }
    text = text.slice(0, low);
    truncated = true;
  }
  if (truncated) {
    text = `${text}\n${t("chat.tool.approval.evidence.truncated")}`;
  }
  return { kind: "content", text, truncated };
}
