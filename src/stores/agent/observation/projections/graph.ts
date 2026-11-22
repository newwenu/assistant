import { currentIterationOf } from "../reducer";
import {
  AGENT_NODE_TEMPLATE,
  TEMPLATE_EDGES,
  type TemplateEdge,
  type TemplateEdgeKind,
} from "../template";
import {
  cellKey,
  type AgentNodeDetail,
  type AgentNodeId,
  type FactSource,
  type NodeState,
  type TurnObservation,
  type TurnObservationStatus,
} from "../types";

/** 一格的可视投影（I55 §6.2）。 */
export interface GraphNodeView {
  id: AgentNodeId;
  order: number;
  row: number;
  column: number;
  /** 迭代外节点恒为 0，渲染时不带圈数徽标（I55 §5.4 第 1 条）。 */
  iteration: number;
  state: NodeState;
  source: FactSource;
  sinceMs: number;
  detail: AgentNodeDetail | null;
  /**
   * 仍停在可覆盖的中间态（`running` / `waiting`）：turn 已收尾时由渲染器显示
   * 「未完成」徽标，**不臆造** `skipped` / `cancelled`（I55 §6.1 收尾规则）。
   */
  unfinished: boolean;
}

/** 边的相位：回边是否发生、正向边是否已走完（I66 §5.2）。 */
export type EdgePhase = "untaken" | "idle" | "passed" | "active";

/** 图上的边：正向边按模板顺序，回边 A 由 ⑫ 回到 ①（I55 §3.1）。 */
export interface GraphEdgeView {
  from: AgentNodeId;
  to: AgentNodeId;
  kind: TemplateEdgeKind;
  phase: EdgePhase;
}

/** 出口标注：只由 `status` 与格子状态推出，不是节点（I66 §5.4）。 */
export interface GraphExitView {
  node: AgentNodeId;
  status: TurnObservationStatus;
}

export interface GraphView {
  conversationId: string;
  turnId: string;
  status: TurnObservationStatus;
  /** 圈数 = 已观测到的最大 `iteration`（`derived`，I55 §5.4 第 3 条）。 */
  iteration: number;
  /** 已观测到的最大 `atMs`：V1 用它算进行中时长（`atMs` 差，不引入时钟）。 */
  nowMs: number;
  nodes: GraphNodeView[];
  /** 12 条边恒定输出，相位由格子状态推出（I66 §5.2）。 */
  edges: GraphEdgeView[];
  /**
   * 当前活动边：`edges` 中相位为 `active` 的那一条（**同一引用**）；
   * 无进行中格子时为 `null`（I66 §5.3）。
   */
  activeEdge: GraphEdgeView | null;
  /** 出口标注；`running` / `waiting` 时为 `null`（I66 §5.4）。 */
  exit: GraphExitView | null;
}

/**
 * 图投影（纯函数，I55 §6.2 / §6.7 末段）。
 *
 * 12 格恒定输出：模板内的格子缺失时补 `pending` + `source = derived`（模板推导，
 * 与 `observed` 不同形渲染，I55 §6.6 第 3 条）；模板外的未知节点不建格子（§4.8 第 3 条）。
 * 出口不是节点（§3.2），只由 `status` 表达。
 */
export function projectGraph(obs: TurnObservation): GraphView {
  const iteration = currentIterationOf(obs);
  const nodes = AGENT_NODE_TEMPLATE.map((entry) => {
    const cellIteration = entry.inIteration ? iteration : 0;
    const cell = obs.cells.get(cellKey(entry.id, cellIteration));
    return {
      id: entry.id,
      order: entry.order,
      row: entry.row,
      column: entry.column,
      iteration: cellIteration,
      state: cell?.state ?? "pending",
      source: cell?.source ?? "derived",
      sinceMs: cell?.sinceMs ?? 0,
      detail: cell?.detail ?? null,
      unfinished: cell ? isUnfinishedState(cell.state) : false,
    } satisfies GraphNodeView;
  });

  const active = activeEdgeOf(nodes, iteration);
  const edges: GraphEdgeView[] = TEMPLATE_EDGES.map((edge) => ({
    ...edge,
    phase: edgePhaseOf(edge, nodes, iteration, active),
  }));

  return {
    conversationId: obs.conversationId,
    turnId: obs.turnId,
    status: obs.status,
    iteration,
    nowMs: observedNowMs(obs),
    nodes,
    edges,
    activeEdge: edges.find((edge) => edge.phase === "active") ?? null,
    exit: exitOf(obs.status, nodes),
  };
}

/** 已观测到的最大 `atMs`：进行中格子的时长取它减 `sinceMs`（I55 §5.3）。 */
function observedNowMs(obs: TurnObservation): number {
  let max = obs.startedAtMs;
  for (const fact of obs.timeline) {
    if (fact.atMs > max) max = fact.atMs;
  }
  for (const cell of obs.cells.values()) {
    if (cell.sinceMs > max) max = cell.sinceMs;
  }
  return max;
}

function isUnfinishedState(state: string): boolean {
  return state === "running" || state === "waiting";
}

/**
 * 活动边判定：取「最靠后的进行中格子」（先比 `iteration`、再比模板顺序），
 * 边指向它。①`admit` 在第二轮及以后由回边 A 进入（I55 §3.1 图示）；
 * `admit` / `prepare` 的 `iteration` 恒为 0（§5.4 第 1 条），故回边用圈数判定。
 */
function activeEdgeOf(
  nodes: readonly GraphNodeView[],
  iteration: number,
): TemplateEdge | null {
  let active: GraphNodeView | null = null;

  for (const node of nodes) {
    if (!node.unfinished) continue;
    if (
      active === null ||
      node.iteration > active.iteration ||
      (node.iteration === active.iteration && node.order > active.order)
    ) {
      active = node;
    }
  }

  if (active === null) return null;
  if (active.id === "admit" && iteration > 1) {
    return TEMPLATE_EDGES.find((edge) => edge.kind === "loop") ?? null;
  }

  return (
    TEMPLATE_EDGES.find(
      (edge) => edge.kind === "forward" && edge.to === active.id,
    ) ?? null
  );
}

/**
 * 边的相位（I66 §5.2）：`active` 由活动边判定结果直接给出；
 * 其余按「格子是否触达」推导，规则表见 I66 §5.2。
 */
export function edgePhaseOf(
  edge: TemplateEdge,
  nodes: readonly GraphNodeView[],
  iteration: number,
  active: TemplateEdge | null,
): EdgePhase {
  if (
    active !== null &&
    edge.from === active.from &&
    edge.to === active.to &&
    edge.kind === active.kind
  ) {
    return "active";
  }

  const from = nodeOf(nodes, edge.from);
  const to = nodeOf(nodes, edge.to);
  if (from === null || to === null) return "untaken";

  if (edge.kind === "loop") {
    if (iteration >= 2) return "passed";
    if (from.state !== "pending") return "idle";
    return "untaken";
  }

  if (to.state === "skipped") return "untaken";
  if (to.state !== "pending") return "passed";
  if (from.state !== "pending") return "idle";
  return "untaken";
}

/** 出口候选：状态 → 可能停在的节点（I66 §5.4 表）。 */
const EXIT_CANDIDATES: Partial<
  Record<TurnObservationStatus, readonly AgentNodeId[]>
> = {
  completed: ["branch"],
  wrap_up: ["branch"],
  halted_by_budget: ["brake"],
  stopped: ["brake", "after-tool", "after-iteration"],
  cancelled: ["ladder", "model", "pipeline"],
  failed: ["ladder", "model", "persist-assistant"],
};

/**
 * 出口标注（I66 §5.4）：候选内先比 `iteration` 降序、再比 `order` 降序，
 * 取首个非 `pending` 格子；全为 `pending` 时取 `order` 最小的候选。
 */
export function exitOf(
  status: TurnObservationStatus,
  nodes: readonly GraphNodeView[],
): GraphExitView | null {
  const candidates = EXIT_CANDIDATES[status];
  if (candidates === undefined) return null;

  const rows = candidates
    .map((id) => nodeOf(nodes, id))
    .filter((node): node is GraphNodeView => node !== null);
  if (rows.length === 0) return null;

  const reached = [...rows]
    .sort((a, b) => b.iteration - a.iteration || b.order - a.order)
    .find((node) => node.state !== "pending");
  const chosen = reached ?? [...rows].sort((a, b) => a.order - b.order)[0];

  return { node: chosen.id, status };
}

function nodeOf(
  nodes: readonly GraphNodeView[],
  id: AgentNodeId,
): GraphNodeView | null {
  return nodes.find((node) => node.id === id) ?? null;
}
