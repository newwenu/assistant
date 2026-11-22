export interface Point {
  x: number;
  y: number;
}

export interface CanvasBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type CardTone =
  "default" | "active" | "success" | "warning" | "danger" | "muted";

export type BadgeTone = "neutral" | "positive" | "warning" | "negative";

export type EdgePhase = "untaken" | "idle" | "passed" | "active";

export type ExitTone = "failed" | "cancelled" | "stopped";

/** 编辑锚点的方位（I68 §6.3）：只是位置与命中区，不承载类型。 */
export type CanvasPortSide = "left" | "right" | "top" | "bottom";

export interface CanvasBadgeView {
  text: string;
  tone?: BadgeTone;
}

/** 阶段条（I68 §7.2）：住在轮次卡片主体里，不是卡片，无头部 / 端口 / 拖拽柄。 */
export interface CanvasStageView {
  id: string;
  /** 模板 `order` 的序号（①②…⑫）。 */
  ordinal: string;
  title: string;
  /** 状态文案（已本地化，非仅靠颜色）。 */
  state: string;
  tone: CardTone;
  /** 职责文案，只进 `title` / `aria-description`。 */
  description: string;
}

/** 主体附加区（I68 §7.3）：轮次卡片放阶段轨道，其余放内容占位。 */
export type CanvasCardBodyView =
  | {
      kind: "stages";
      label: string;
      count: string;
      stages: readonly CanvasStageView[];
    }
  | { kind: "output"; label: string }
  | { kind: "messages"; label: string }
  | { kind: "question"; label: string }
  | { kind: "subgraph"; label: string; nodes: string };

/** 参数摘要的一行（只读，无控件、无校验）。 */
export interface CanvasCardParamView {
  label: string;
  value: string;
}

/** 占位端口（I68 §6.3）：圆点 + 占位类型标签，不承载类型、不校验。 */
export interface CanvasCardPortView {
  side: CanvasPortSide;
  typeLabel: string;
}

export interface CanvasCardView {
  id: string;
  box: CanvasBox;
  /** 头部：类别徽标（`canvas.card.category.*` + 分型名）。 */
  kindBadge: string;
  /** 头部：卡片名称。 */
  title: string;
  /** 头部右端「示例」徽标；省略 = 不画。 */
  exampleBadge?: string;
  /** 主体：参数摘要（3 行只读键值对）。 */
  params?: readonly CanvasCardParamView[];
  /** 主体：附加区。 */
  body: CanvasCardBodyView;
  /** 端口区：占位端口（圆点 + 占位类型标签）。 */
  ports?: readonly CanvasCardPortView[];
  /** 脚部：状态药丸。 */
  status?: string;
  /** 脚部：引用 chip（前缀 + 原语量）。 */
  reference?: string;
  /** 脚部：时长等原语量。 */
  measure?: string;
  tone?: CardTone;
  /** 右下角出口角标；仅轮次卡片。 */
  exit?: ExitTone | null;
  /** 容器卡片：虚线边框（示例标记，I68 §7.1）。 */
  container?: boolean;
}

export type EdgeRoute = "horizontal" | "vertical" | "outer-track";

export interface CanvasEdgeView {
  id: string;
  from: string;
  to: string;
  route: EdgeRoute;
  phase: EdgePhase;
}

export interface CanvasLabelView {
  id: string;
  box: CanvasBox;
  text: string;
  tone?: "muted" | "default";
}
