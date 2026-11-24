/**
 * 叶子纯函数：参数读取、行 / 字节口径、显示宽度与中间省略。
 *
 * 只做「怎么数、怎么量、怎么截」，不含任何审批语义；被要素求值与证据块共用。
 */

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function readBoolean(
  args: Record<string, unknown>,
  key: string,
): boolean | undefined {
  const value = args[key];
  return typeof value === "boolean" ? value : undefined;
}

export function nonEmptyString(value: unknown): string | null {
  if (typeof value !== "string") return null;
  return value.trim() === "" ? null : value;
}

/** Rust `str::lines()` 口径：末尾换行不产生空行，空串为零行。 */
export function splitLines(text: string): string[] {
  const normalized = text.replace(/\r\n/g, "\n");
  if (normalized === "") return [];
  const lines = normalized.split("\n");
  if (normalized.endsWith("\n")) lines.pop();
  return lines;
}

export function lineCount(text: string): number {
  return splitLines(text).length;
}

export function utf8Length(text: string): number {
  return new TextEncoder().encode(text).length;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(1)} ${units[index]}`;
}

export function displayWidth(text: string): number {
  let width = 0;
  for (const char of text) {
    width += char.charCodeAt(0) > 0x2e80 ? 2 : 1;
  }
  return width;
}

/** 中间省略、保留首尾（§4.2 对象段 / §8.2 命令项）。 */
export function middleEllipsis(text: string, maxWidth: number): string {
  if (displayWidth(text) <= maxWidth) return text;
  const headWidth = Math.floor(maxWidth * 0.6);
  const tailWidth = Math.floor(maxWidth * 0.35);
  const chars = Array.from(text);
  let head = "";
  let width = 0;
  for (const char of chars) {
    const charWidth = char.charCodeAt(0) > 0x2e80 ? 2 : 1;
    if (width + charWidth > headWidth) break;
    head += char;
    width += charWidth;
  }
  let tail = "";
  width = 0;
  for (const char of [...chars].reverse()) {
    const charWidth = char.charCodeAt(0) > 0x2e80 ? 2 : 1;
    if (width + charWidth > tailWidth) break;
    tail = char + tail;
    width += charWidth;
  }
  return `${head}…${tail}`;
}

/** 控制操作符（结构层事实）：顺序 / 条件 / 后台 / 管道 / 重定向 / 换行 / 子命令（§6.3）。 */
export function hasControlOperator(command: string): boolean {
  return /[;&|<>`]|\$\(|\r|\n/.test(command);
}

export function commandText(args: Record<string, unknown>): string | null {
  const value = args.command;
  if (typeof value === "string") return nonEmptyString(value);
  if (Array.isArray(value)) {
    const parts = value
      .filter((part): part is string => typeof part === "string")
      .map((part) => part.trim())
      .filter((part) => part !== "");
    return parts.length > 0 ? parts.join(" ") : null;
  }
  return null;
}

/** 折叠区「全部参数」原文（§4.5 第 4 区块）。 */
export function argumentsText(args: Record<string, unknown>): string {
  if (Object.keys(args).length === 0) return "{}";
  return JSON.stringify(args, null, 2);
}
