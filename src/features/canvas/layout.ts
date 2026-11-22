import type { CanvasBox } from "./types";

export interface CanvasLayoutEntry {
  id: string;
  row: number;
  column: number;
}

/**
 * 摆放度量（I68 §4.3）：几何公式只认这四个字段，
 * 观测栅格用 `CANVAS_METRICS`，画布卡片用 `CARD_METRICS`，两者互不引用。
 */
export interface CanvasMetrics {
  cellW: number;
  cellH: number;
  gap: number;
  loopGutter: number;
  gridMinor?: number;
  gridMajor?: number;
}

export const CANVAS_METRICS = {
  cellW: 132,
  cellH: 78,
  gap: 8,
  loopGutter: 16,
  gridMinor: 40,
  gridMajor: 200,
} as const satisfies CanvasMetrics;

export interface CanvasLayout {
  boxes: ReadonlyMap<string, CanvasBox>;
  width: number;
  height: number;
}

export function layoutTemplate(
  entries: readonly CanvasLayoutEntry[],
  metrics: CanvasMetrics = CANVAS_METRICS,
): CanvasLayout {
  const boxes = new Map<string, CanvasBox>();
  if (entries.length === 0) {
    return { boxes, width: 0, height: 0 };
  }

  const { cellW, cellH, gap, loopGutter } = metrics;
  let maxColumn = 0;
  let maxRow = 0;

  for (const entry of entries) {
    boxes.set(entry.id, {
      x: entry.column * (cellW + gap),
      y: loopGutter + gap + entry.row * (cellH + gap),
      w: cellW,
      h: cellH,
    });
    if (entry.column > maxColumn) maxColumn = entry.column;
    if (entry.row > maxRow) maxRow = entry.row;
  }

  return {
    boxes,
    width: (maxColumn + 1) * cellW + maxColumn * gap,
    height: loopGutter + gap + (maxRow + 1) * cellH + maxRow * gap,
  };
}
