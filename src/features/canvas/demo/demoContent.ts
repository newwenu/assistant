import { CANVAS_METRICS, layoutTemplate } from "../layout";
import type { CanvasBox, CanvasEdgeView, ExitTone } from "../types";

/**
 * 示例内容（§11）：W1 自检设施用，只有 `demo-*` 假标识，整体可删除。
 * 布局在模块作用域算一次，是调用方常量（§12.2）。
 */
const DEMO_ENTRIES = [
  { id: "demo-a", row: 0, column: 0 },
  { id: "demo-b", row: 0, column: 1 },
  { id: "demo-c", row: 0, column: 2 },
  { id: "demo-d", row: 0, column: 3 },
  { id: "demo-e", row: 1, column: 3 },
  { id: "demo-f", row: 1, column: 4 },
] as const;

export const DEMO_LAYOUT = layoutTemplate(DEMO_ENTRIES);

export interface DemoCardDescriptor {
  id: string;
  titleKey: string;
  subtitleKey: string;
  exit?: ExitTone | null;
}

export const DEMO_CARDS = [
  {
    id: "demo-a",
    titleKey: "canvas.demo.card.a",
    subtitleKey: "canvas.demo.subtitle.a",
    exit: null,
  },
  {
    id: "demo-b",
    titleKey: "canvas.demo.card.b",
    subtitleKey: "canvas.demo.subtitle.b",
    exit: null,
  },
  {
    id: "demo-c",
    titleKey: "canvas.demo.card.c",
    subtitleKey: "canvas.demo.subtitle.c",
    exit: null,
  },
  {
    id: "demo-d",
    titleKey: "canvas.demo.card.d",
    subtitleKey: "canvas.demo.subtitle.d",
    exit: "stopped",
  },
  {
    id: "demo-e",
    titleKey: "canvas.demo.card.e",
    subtitleKey: "canvas.demo.subtitle.e",
    exit: null,
  },
  {
    id: "demo-f",
    titleKey: "canvas.demo.card.f",
    subtitleKey: "canvas.demo.subtitle.f",
    exit: null,
  },
] as const satisfies readonly DemoCardDescriptor[];

export const DEMO_EDGES: readonly CanvasEdgeView[] = [
  {
    id: "demo-edge-ab",
    from: "demo-a",
    to: "demo-b",
    route: "horizontal",
    phase: "untaken",
  },
  {
    id: "demo-edge-bc",
    from: "demo-b",
    to: "demo-c",
    route: "horizontal",
    phase: "idle",
  },
  {
    id: "demo-edge-cd",
    from: "demo-c",
    to: "demo-d",
    route: "horizontal",
    phase: "passed",
  },
  {
    id: "demo-edge-de",
    from: "demo-d",
    to: "demo-e",
    route: "vertical",
    phase: "active",
  },
];

const { cellW, cellH, gap, loopGutter } = CANVAS_METRICS;

/** 第二行左侧留白（列 0–2）放一条标签，覆盖 overlays 层。 */
export const DEMO_LABEL_BOX: CanvasBox = {
  x: 0,
  y: loopGutter + gap + cellH + gap,
  w: cellW * 3 + gap * 2,
  h: cellH,
};

export const DEMO_LABEL_KEY = "canvas.demo.lane";
