import { currentIterationOf } from "../reducer";
import { AGENT_NODE_TEMPLATE, templateOrderOf } from "../template";
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

/** 图上的边：正向边按模板顺序，回边 A 由 ⑫ 回到 ①（I55 §3.1）。 */
export interface GraphEdgeView {
  from: AgentNodeId;
  to: AgentNodeId;
  kind: "forward" | "loop";
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
  /** 当前活动边：进入「最靠后的进行中格子」的那条边；无进行中格子为 `null`。 */
  activeEdge: GraphEdgeView | null;
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

  return {
    conversationId: obs.conversationId,
    turnId: obs.turnId,
    status: obs.status,
    iteration,
    nowMs: observedNowMs(obs),
    nodes,
    activeEdge: activeEdgeOf(obs, iteration),
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
  obs: TurnObservation,
  iteration: number,
): GraphEdgeView | null {
  let active: { id: AgentNodeId; iteration: number; order: number } | null =
    null;

  for (const entry of AGENT_NODE_TEMPLATE) {
    const cellIteration = entry.inIteration ? iteration : 0;
    const cell = obs.cells.get(cellKey(entry.id, cellIteration));
    if (!cell || !isUnfinishedState(cell.state)) continue;
    if (
      active === null ||
      cell.iteration > active.iteration ||
      (cell.iteration === active.iteration && entry.order > active.order)
    ) {
      active = { id: entry.id, iteration: cell.iteration, order: entry.order };
    }
  }

  if (active === null) return null;
  if (active.id === "admit" && iteration > 1) {
    return { from: "after-iteration", to: "admit", kind: "loop" };
  }

  const predecessor = templatePredecessor(active.id);
  if (predecessor === null) return null;
  return { from: predecessor, to: active.id, kind: "forward" };
}

function templatePredecessor(node: AgentNodeId): AgentNodeId | null {
  const order = templateOrderOf(node);
  if (order === null || order === 0) return null;
  return (
    AGENT_NODE_TEMPLATE.find((entry) => entry.order === order - 1)?.id ?? null
  );
}
