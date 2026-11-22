import type { CanvasMetrics } from "../layout";

/**
 * 画布卡片度量（I68 §4.3 / §7.4）：卡片设计的尺寸，与观测栅格的
 * `CANVAS_METRICS`（132×78）无关，二者不得互相引用（A19）。
 * 字段名沿用 `CanvasMetrics` 的「单元宽高」抽象，取值由卡片设计决定：
 * `32 + (66 + 1 + 115) + 26 = 240`（头部 / 参数摘要 / 分隔线 / 附加区 / 脚部）。
 * `loopGutter = 0`：画布上没有回边，不再需要顶部外轨（T-26）。
 */
export const CARD_METRICS = {
  cellW: 320,
  cellH: 240,
  gap: 32,
  loopGutter: 0,
} as const satisfies CanvasMetrics;
