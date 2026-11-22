import type { TFunction } from "i18next";
import {
  BUILTIN_AGENT_LAYOUT,
  BUILTIN_AGENT_PRESET,
} from "@/features/canvas/presets/builtinAgentPreset";
import { CARD_KINDS, CARD_KIND_IDS } from "@/features/canvas/presets/cardKinds";
import type { CanvasPaletteItem } from "@/features/canvas/CanvasPalette";
import { zoneLabelBoxOf } from "@/features/canvas/presets/placement";
import type { CardKindId, CardKindSpec } from "@/features/canvas/presets/types";
import type {
  CanvasCardBodyView,
  CanvasCardPortView,
  CanvasCardView,
  CanvasEdgeView,
  CanvasLabelView,
  CanvasPortSide,
  CanvasStageView,
  CardTone,
  ExitTone,
} from "@/features/canvas/types";
import type { GraphView } from "@/stores/agent/observation/projections/graph";
import { AGENT_NODE_TEMPLATE } from "@/stores/agent/observation/template";
import type {
  AgentNodeId,
  NodeState,
  TurnObservationStatus,
} from "@/stores/agent/observation/types";

/** 模板顺序徽标：与 `AgentNodeTemplateEntry.order` 一一对应（0 起）。 */
const ORDER_BADGES = [
  "①",
  "②",
  "③",
  "④",
  "⑤",
  "⑥",
  "⑦",
  "⑧",
  "⑨",
  "⑩",
  "⑪",
  "⑫",
] as const;

/** 节点状态 → 阶段条色调（设施只认 `CardTone`，不认 `NodeState`）。 */
const STAGE_TONE: Record<NodeState, CardTone> = {
  pending: "default",
  running: "active",
  waiting: "warning",
  finished: "success",
  error: "danger",
  skipped: "muted",
  cancelled: "muted",
};

/** 实例终态 → 出口标记；`running` 与正常收尾不挂出口（I66 §5.4）。 */
const EXIT_TONE: Record<TurnObservationStatus, ExitTone | null> = {
  running: null,
  completed: null,
  stopped: "stopped",
  halted_by_budget: "stopped",
  wrap_up: "stopped",
  cancelled: "cancelled",
  failed: "failed",
};

/** 端口区渲染顺序：左 → 右 → 上 → 下，保证跨投影稳定（I68 §7.2）。 */
const PORT_ORDER: readonly CanvasPortSide[] = [
  "left",
  "right",
  "top",
  "bottom",
];

/**
 * 内容层映射（I68 §7）：预设卡片 → 画布视图模型。
 *
 * 设施不认识观测域，所以「5 张示例卡片长什么样」全部在这里决定；几何只查
 * `BUILTIN_AGENT_LAYOUT`（由预设经 `layoutTemplate` 算出，模块加载时一次）。
 * 卡片形与示例内容来自 `CARD_KINDS`，唯一来自运行的内容是阶段状态。
 * `graph` 为 `null` 时输出模板预览：阶段全部 `pending`，连线全部 `idle`。
 */
export function canvasContentOf(
  graph: GraphView | null,
  t: TFunction,
): {
  cards: readonly CanvasCardView[];
  edges: readonly CanvasEdgeView[];
  labels: readonly CanvasLabelView[];
} {
  const states = statesOf(graph);
  return {
    cards: BUILTIN_AGENT_PRESET.cards.map((card) =>
      cardViewOf(card.kind, states, graph, t),
    ),
    edges: BUILTIN_AGENT_PRESET.connectors.map((connector) => ({
      id: `${connector.from}->${connector.to}`,
      from: connector.from,
      to: connector.to,
      route: connector.route,
      phase: "idle" as const,
    })),
    labels: labelsOf(t),
  };
}

/**
 * 放置栏落卡用的模板视图（I68 §7.10 / A24）：与该类别在画布上的卡片同形同内容。
 * 盒子取预设初始盒子，调用方负责改 id 与落点（放置不进摆放函数）。
 */
export function exampleCardOf(kind: CardKindId, t: TFunction): CanvasCardView {
  return cardViewOf(kind, new Map(), null, t);
}

/**
 * 放置栏条目（I68 §7.10 / A24）：与画布卡片共用 `CARD_KINDS` 与同一套徽标读法，
 * 顺序即类别表声明顺序（执行卡片四型 → 容器卡片）。
 */
export function paletteItemsOf(t: TFunction): CanvasPaletteItem[] {
  return CARD_KIND_IDS.map((kind) => {
    const spec = CARD_KINDS[kind];
    return {
      kind,
      badge: kindBadgeOf(spec, t),
      name: t(spec.nameKey),
      summary: t(spec.summaryKey),
    };
  });
}

interface NodeStateEntry {
  state: NodeState;
}

function statesOf(
  graph: GraphView | null,
): ReadonlyMap<AgentNodeId, NodeStateEntry> {
  if (graph === null) return new Map();
  return new Map(graph.nodes.map((node) => [node.id, { state: node.state }]));
}

function cardViewOf(
  kind: CardKindId,
  states: ReadonlyMap<AgentNodeId, NodeStateEntry>,
  graph: GraphView | null,
  t: TFunction,
): CanvasCardView {
  const spec = CARD_KINDS[kind];
  const box = BUILTIN_AGENT_LAYOUT.boxes.get(kind);
  if (box === undefined) {
    throw new Error(`canvas content: preset has no box for ${kind}`);
  }

  return {
    id: kind,
    box,
    kindBadge: kindBadgeOf(spec, t),
    title: t(spec.nameKey),
    exampleBadge: t("canvas.card.badge_example"),
    params: spec.params.map((param) => ({
      label: t(param.labelKey),
      value: param.value,
    })),
    body: bodyOf(spec, states, t),
    ports: portsOf(kind, spec),
    status: statusOf(kind, graph, t),
    reference: `${t("canvas.card.reference")} ${spec.reference}`,
    measure: kind === "execution-turn" ? "1.2s" : undefined,
    tone: spec.tone,
    exit: kind === "execution-turn" ? turnExitOf(graph) : null,
    container: spec.category === "container",
  };
}

/** 轮次卡片出口：有投影按运行状态取，无投影沿用类别表的示例出口。 */
function turnExitOf(graph: GraphView | null): ExitTone | null {
  if (graph === null) return CARD_KINDS["execution-turn"].exit;
  return EXIT_TONE[graph.status];
}

/** 徽标 = 类别 + 分型（容器卡片不分型，只有类别）。 */
function kindBadgeOf(spec: CardKindSpec, t: TFunction): string {
  const category = t(`canvas.card.category.${spec.category}`);
  return spec.formKey === null ? category : `${category} · ${t(spec.formKey)}`;
}

function bodyOf(
  spec: CardKindSpec,
  states: ReadonlyMap<AgentNodeId, NodeStateEntry>,
  t: TFunction,
): CanvasCardBodyView {
  switch (spec.body) {
    case "stages":
      return {
        kind: "stages",
        label: t("canvas.card.stages"),
        count: String(AGENT_NODE_TEMPLATE.length),
        stages: stagesOf(states, t),
      };
    case "subgraph":
      return {
        kind: "subgraph",
        label: t("canvas.card.body.subgraph"),
        nodes: spec.params[0]?.value ?? "",
      };
    case "output":
      return { kind: "output", label: t("canvas.card.body.output") };
    case "messages":
      return { kind: "messages", label: t("canvas.card.body.messages") };
    case "question":
      return { kind: "question", label: t("canvas.card.body.question") };
  }
}

/** 阶段轨道（A23）：12 条阶段条与模板 `order` 一一对应；无投影时全部「未开始」。 */
function stagesOf(
  states: ReadonlyMap<AgentNodeId, NodeStateEntry>,
  t: TFunction,
): CanvasStageView[] {
  return AGENT_NODE_TEMPLATE.map((entry) => {
    const state = states.get(entry.id)?.state ?? "pending";
    return {
      id: entry.id,
      ordinal: ORDER_BADGES[entry.order] ?? "",
      title: t(`observation.node.${entry.id}`),
      state: t(`observation.state.${state}`),
      tone: STAGE_TONE[state],
      description: t(`observation.role.${entry.id}`),
    };
  });
}

/**
 * 占位端口：类别表给左入 / 右出；跨行连线的两端补下出 / 上入（I68 §6.3 / §7.3）。
 * 类型标签是原语量占位，不翻译、不校验。
 */
function portsOf(kind: CardKindId, spec: CardKindSpec): CanvasCardPortView[] {
  const ports: CanvasCardPortView[] = spec.ports.map((port) => ({
    side: port.side,
    typeLabel: port.typeLabel,
  }));

  for (const connector of BUILTIN_AGENT_PRESET.connectors) {
    if (connector.route !== "vertical") continue;
    if (connector.from === kind) {
      ports.push({
        side: "bottom",
        typeLabel: lastLabelOf(spec, "right") ?? "",
      });
    }
    if (connector.to === kind) {
      ports.push({
        side: "top",
        typeLabel: lastLabelOf(spec, "left") ?? "",
      });
    }
  }

  return PORT_ORDER.flatMap((side) =>
    ports.filter((port) => port.side === side),
  );
}

function lastLabelOf(
  spec: CardKindSpec,
  side: "left" | "right",
): string | undefined {
  const labels = spec.ports.filter((port) => port.side === side);
  return labels[labels.length - 1]?.typeLabel;
}

/** 脚部状态药丸：只有轮次卡片接运行状态，其余是示例卡片的固定「未开始」。 */
function statusOf(
  kind: CardKindId,
  graph: GraphView | null,
  t: TFunction,
): string {
  if (kind === "execution-turn" && graph !== null) {
    return t(`observation.status.${graph.status}`);
  }
  return t("observation.state.pending");
}

/** 区带标签：只有预设给了标签列位（且给了文案键）的区带才画。 */
function labelsOf(t: TFunction): CanvasLabelView[] {
  return BUILTIN_AGENT_PRESET.zones.flatMap((zone) => {
    if (zone.labelKey === null) return [];
    const box = zoneLabelBoxOf(zone);
    if (box === null) return [];
    return [
      {
        id: `zone-${zone.id}`,
        box,
        text: t(zone.labelKey),
        tone: "muted" as const,
      },
    ];
  });
}
