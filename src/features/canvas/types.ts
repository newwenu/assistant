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

/**
 * 显示阶段：一个显示阶段聚合一至多个 L1 节点。
 * 聚合与计数在内容层算好，卡片只读取（A13）。
 */
export interface CanvasPhaseView {
  /** 显示阶段 id（`settle` / `decide` / …）。 */
  id: string;
  /** 轨道序号（1 起，与归并表顺序一致）。 */
  ordinal: string;
  /** 显示阶段名（已本地化）。 */
  title: string;
  /** 聚合状态色。 */
  tone: CardTone;
  /** 已闭合 L1 数（终态才算闭合）。 */
  closed: number;
  /** L1 总数。 */
  total: number;
  /** 组内 L1 明细，只进 `title`（不单独排版）。 */
  detail: string;
}

/** 主体附加区：轮次卡片放显示阶段轨道，其余放内容占位。 */
export type CanvasCardBodyView =
  | {
      kind: "phases";
      label: string;
      count: string;
      phases: readonly CanvasPhaseView[];
    }
  | { kind: "output"; label: string }
  | { kind: "messages"; label: string }
  | { kind: "question"; label: string }
  | { kind: "subgraph"; label: string; nodes: string }
  | {
      kind: "detail";
      label: string;
      rows: readonly { label: string; value: string }[];
      emptyHint?: string;
    };

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
  /** 可选虚线框（I69d INC3）：`true` 时画虚线边框 + 左上标题；缺省为纯文字标签。 */
  frame?: boolean;
}
