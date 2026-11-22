/**
 * 预设摆放（I68 §5）：预设 → 布局条目 → W1 `layoutTemplate`，坐标公式只有一份。
 * 放在内容层而非 `layout.ts`，是因为设施层不认预设；本模块只读预设，不读模板（A9）。
 */
import {
  layoutTemplate,
  type CanvasLayout,
  type CanvasLayoutEntry,
} from "../layout";
import type { CanvasBox } from "../types";
import { CARD_METRICS } from "./metrics";
import type { CanvasPreset, CanvasZone, CanvasZoneId } from "./types";

/** 预设 → 初始盒子：行号来自区带，列位来自卡片声明，度量来自 `CARD_METRICS`。 */
export function layoutPreset(preset: CanvasPreset): CanvasLayout {
  const rowByZone = new Map<CanvasZoneId, number>(
    preset.zones.map((zone) => [zone.id, zone.row]),
  );
  const entries = preset.cards.map((card): CanvasLayoutEntry => {
    const row = rowByZone.get(card.zone);
    if (row === undefined) {
      throw new Error(`canvas preset: unknown zone ${card.zone}`);
    }
    return { id: card.kind, row, column: card.column };
  });
  return layoutTemplate(entries, CARD_METRICS);
}

/** 区带标签的盒子：只覆盖 `labelColumns` 的留白列，不参与卡片几何。 */
export function zoneLabelBoxOf(zone: CanvasZone): CanvasBox | null {
  if (zone.labelColumns === null || zone.labelColumns.length === 0) {
    return null;
  }
  const { cellW, cellH, gap, loopGutter } = CARD_METRICS;
  const first = Math.min(...zone.labelColumns);
  const last = Math.max(...zone.labelColumns);
  const span = last - first;
  return {
    x: first * (cellW + gap),
    y: loopGutter + gap + zone.row * (cellH + gap),
    w: (span + 1) * cellW + span * gap,
    h: cellH,
  };
}
