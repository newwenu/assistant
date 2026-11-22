import type { CanvasMetrics } from "../layout";

/**
 * 画布卡片度量：卡片设计的尺寸，与观测栅格的 `CANVAS_METRICS`（132×78）
 * 无关，二者不得互相引用（A19）。字段名沿用 `CanvasMetrics` 的「单元宽高」抽象：
 * 竖高于宽，纵向预算 `38 + 8 + 54 + 1 + 130 + 8 + 1 + 26 = 266 ≤ 360`，
 * 余量由附加区吸收（阶段轨道固定高，留白不摊到阶段行）。
 * `loopGutter = 0`：画布上没有回边，不再需要顶部外轨（T-26）。
 */
export const CARD_METRICS = {
  cellW: 300,
  cellH: 360,
  gap: 32,
  loopGutter: 0,
} as const satisfies CanvasMetrics;

/**
 * 端口圆点度量：每侧一个业务锚点（边框中点），同侧 N 个圆点按
 * `spreadStep` 围绕中心展开，只为避免视觉重叠；连线仍接中点。
 */
export const PORT_METRICS = {
  spreadStep: 16,
  sideMax: 4,
  horizontalMax: 5,
  dotSize: 8,
} as const;

/** 同侧第 `index` 个圆点相对中点的偏移（像素）：`(i - (N - 1) / 2) × step`。 */
export function portSpreadOffset(index: number, count: number): number {
  return (index - (count - 1) / 2) * PORT_METRICS.spreadStep;
}
