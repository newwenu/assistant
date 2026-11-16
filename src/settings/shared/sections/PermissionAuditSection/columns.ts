import type { CSSProperties } from "react";

const COLUMN_KEYS = [
  "time",
  "turn",
  "tool",
  "action",
  "resource",
  "outcome",
] as const;

export type ColumnKey = (typeof COLUMN_KEYS)[number];

export { COLUMN_KEYS };

export const LIST_MAX_HEIGHT = 320;
export const COLUMN_MIN_WIDTH = 64;
export const COLUMN_MAX_WIDTH = 420;
export const COLUMN_GUTTER = 8;
export const RESIZE_HANDLE_WIDTH = 8;
const COLUMN_BORDER = "1px solid var(--color-border-secondary)";

export const DEFAULT_COLUMN_WIDTHS: Record<ColumnKey, number> = {
  time: 132,
  turn: 168,
  tool: 104,
  action: 104,
  resource: 232,
  outcome: 124,
};

export function clampColumnWidth(width: number): number {
  return Math.min(COLUMN_MAX_WIDTH, Math.max(COLUMN_MIN_WIDTH, width));
}

/**
 * 单元格容器：负责分界线与内边距，让 `grid` 轨道边界即视觉分界。
 * `minWidth: 0` 是省略号生效的前提（默认 `auto` 会撑破轨道）。
 */
export function columnCellStyle(isLast: boolean): CSSProperties {
  return {
    position: "relative",
    boxSizing: "border-box",
    display: "flex",
    alignItems: "center",
    minWidth: 0,
    paddingInlineStart: COLUMN_GUTTER,
    paddingInlineEnd: isLast ? 0 : COLUMN_GUTTER,
    borderInlineEnd: isLast ? undefined : COLUMN_BORDER,
  };
}

export const ELLIPSIS_TEXT_STYLE: CSSProperties = {
  minWidth: 0,
  flex: "1 1 auto",
};

export const CLIPPED_TAG_STYLE: CSSProperties = {
  display: "inline-block",
  marginInlineEnd: 0,
  minWidth: 0,
  maxWidth: "100%",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};
