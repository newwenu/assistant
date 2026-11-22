import type { AgentNodeId, NodeState } from "./types";

/** 模板条目：id / 行 / 列 / 是否在迭代内（I55 §3.1）。 */
export interface AgentNodeTemplateEntry {
  id: AgentNodeId;
  /** 模板顺序（0 起）：用于推导「同迭代中排在它后面的节点仍是 pending」。 */
  order: number;
  row: number;
  column: number;
  /** 迭代内节点从 1 起；迭代外节点（`admit` / `prepare`）恒为 0（I55 §5.4 第 1 条）。 */
  inIteration: boolean;
}

/**
 * 12 节点静态模板（I55 §3.1）。
 *
 * **模板不是数据**（O4）：它是编译期常量，不进 wire、无运行时注册表。
 * `satisfies Record<AgentNodeDto, …>` 是 G2 的编译期锁：缺一个节点或多一个节点
 * 都会直接编译失败，因此不需要（也不应该有）一份「运行时节点表」。
 */
const TEMPLATE_BY_ID = {
  admit: { order: 0, row: 0, column: 0, inIteration: false },
  prepare: { order: 1, row: 0, column: 1, inIteration: false },
  brake: { order: 2, row: 0, column: 2, inIteration: true },
  ladder: { order: 3, row: 0, column: 3, inIteration: true },
  model: { order: 4, row: 0, column: 4, inIteration: true },
  "persist-assistant": { order: 5, row: 0, column: 5, inIteration: true },
  branch: { order: 6, row: 0, column: 6, inIteration: true },
  "budget-split": { order: 7, row: 1, column: 6, inIteration: true },
  pipeline: { order: 8, row: 1, column: 7, inIteration: true },
  "after-tool": { order: 9, row: 1, column: 8, inIteration: true },
  "persist-tools": { order: 10, row: 1, column: 9, inIteration: true },
  "after-iteration": { order: 11, row: 1, column: 10, inIteration: true },
} satisfies Record<AgentNodeId, Omit<AgentNodeTemplateEntry, "id">>;

/** 模板常量（顺序 = `order`，即 I55 §3.1 的行内顺序）。 */
export const AGENT_NODE_TEMPLATE: readonly AgentNodeTemplateEntry[] = (
  Object.keys(TEMPLATE_BY_ID) as AgentNodeId[]
).map((id) => ({ id, ...TEMPLATE_BY_ID[id] }));

/** 12 个节点 id 的模板序列表。 */
export const AGENT_NODE_IDS: readonly AgentNodeId[] = AGENT_NODE_TEMPLATE.map(
  (entry) => entry.id,
);

/** 边的种类：正向（`order` 相邻）或回边（⑫ → ①）。 */
export type TemplateEdgeKind = "forward" | "loop";

/** 模板边：全图边集的唯一来源（I66 §5.1）。 */
export interface TemplateEdge {
  from: AgentNodeId;
  to: AgentNodeId;
  kind: TemplateEdgeKind;
}

const ID_BY_ORDER = new Map<number, AgentNodeId>(
  AGENT_NODE_TEMPLATE.map((entry) => [entry.order, entry.id]),
);

const FORWARD_EDGES: readonly TemplateEdge[] = AGENT_NODE_TEMPLATE.flatMap(
  (entry): TemplateEdge[] => {
    const previous = ID_BY_ORDER.get(entry.order - 1);
    if (previous === undefined) return [];
    return [{ from: previous, to: entry.id, kind: "forward" }];
  },
);

/** 12 条边：11 条正向（`order` 相邻）+ 1 条回边 A（⑫ → ①，I55 §3.1）。 */
export const TEMPLATE_EDGES = [
  ...FORWARD_EDGES,
  { from: "after-iteration", to: "admit", kind: "loop" },
] satisfies readonly TemplateEdge[];

/** 有工具调用时才会走到的后半段节点（⑦ 判定后由 `BranchDetail` 批量标 `skipped`）。 */
export const TOOL_BRANCH_NODE_IDS: readonly AgentNodeId[] = [
  "budget-split",
  "pipeline",
  "after-tool",
  "persist-tools",
  "after-iteration",
];

/** 7 值状态闭集（I55 §4.3）。 */
export const NODE_STATES: readonly NodeState[] = [
  "pending",
  "running",
  "waiting",
  "finished",
  "error",
  "skipped",
  "cancelled",
];

/** 4 个终态（I55 §4.3）：写入后不可再被覆盖（O2）。 */
export const TERMINAL_NODE_STATES: readonly NodeState[] = [
  "finished",
  "error",
  "skipped",
  "cancelled",
];

const TEMPLATE_ENTRY_BY_ID = new Map<AgentNodeId, AgentNodeTemplateEntry>(
  AGENT_NODE_TEMPLATE.map((entry) => [entry.id, entry]),
);

const ORDER_BY_ID = new Map<AgentNodeId, number>(
  AGENT_NODE_TEMPLATE.map((entry) => [entry.id, entry.order]),
);

const STATE_SET = new Set<string>(NODE_STATES);

/** 未知 node 取值不建格子，只进时间线（I55 §4.8 第 3 条）。 */
export function isAgentNodeId(value: string): value is AgentNodeId {
  return ORDER_BY_ID.has(value as AgentNodeId);
}

/** 未知 state 取值不建格子，只进时间线（I55 §4.8 第 3 条）。 */
export function isNodeState(value: string): value is NodeState {
  return STATE_SET.has(value);
}

/** 终态判定：`finished` / `error` / `skipped` / `cancelled`。 */
export function isTerminalNodeState(state: string): boolean {
  return (TERMINAL_NODE_STATES as readonly string[]).includes(state);
}

/** 模板顺序；未知节点返回 `null`（调用方据此跳过推导）。 */
export function templateOrderOf(node: string): number | null {
  const order = ORDER_BY_ID.get(node as AgentNodeId);
  return order === undefined ? null : order;
}

/** 该节点是否在迭代内（决定 `iteration = 0` 还是从 1 起）。 */
export function isIterationNode(node: AgentNodeId): boolean {
  return TEMPLATE_ENTRY_BY_ID.get(node)?.inIteration ?? false;
}

/** 模板顺序之后、且同在迭代内的节点（`pending` 推导的目标集合）。 */
export function nodesAfterInIteration(node: AgentNodeId): AgentNodeId[] {
  const order = templateOrderOf(node);
  if (order === null) return [];
  return AGENT_NODE_TEMPLATE.filter(
    (entry) => entry.inIteration && entry.order > order,
  ).map((entry) => entry.id);
}
