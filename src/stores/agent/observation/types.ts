import type {
  AgentNodeDetailDto,
  AgentNodeDto,
  NodeStateDto,
} from "../../../types/protocol.contract";

/** 12 个节点 id（I55 §3.1），与契约生成类型同源。 */
export type AgentNodeId = AgentNodeDto;

/** 7 值状态闭集（I55 §4.3）。 */
export type NodeState = NodeStateDto;

/** 按 node 分派的 D 级细节闭集（I55 §4.4）。 */
export type AgentNodeDetail = AgentNodeDetailDto;

/**
 * 格子的证据等级（I55 §6.6）。
 *
 * 完全由前端本地推导，**不进 wire**（O4）：`observed` 来自真实事件，
 * `derived` 来自静态模板 / `BranchDetail` / `max(iteration)`，
 * `assumed` 来自窗口重载与切会话的补拉。
 */
export type FactSource = "observed" | "derived" | "assumed";

/** 证据等级强弱：只升不降（I55 §6.6 第 2 条）。 */
export const FACT_SOURCE_RANK: Record<FactSource, number> = {
  derived: 0,
  assumed: 1,
  observed: 2,
};

/** 实例级终态（I55 §6.1）：出口本身不是节点（I55 §3.2）。 */
export type TurnObservationStatus =
  | "running"
  | "completed"
  | "stopped"
  | "halted_by_budget"
  | "wrap_up"
  | "cancelled"
  | "failed";

/** 一个 `(node, iteration)` 格子的当前状态。 */
export interface NodeCell {
  node: AgentNodeId;
  iteration: number;
  state: NodeState;
  /**
   * 进入当前状态（`running` / `waiting`）的时刻，相对 turn 起点毫秒。
   * `running ⇄ waiting` 互转**不重置**它（I55 §5.3 前提 ②）。
   */
  sinceMs: number;
  detail: AgentNodeDetail | null;
  source: FactSource;
}

/**
 * 时间线上的一条事实（I55 §6.2 inspector 输入）。
 *
 * `node` / `state` 用 `string`：未知取值必须保留到时间线（I55 §4.8 第 3 条），
 * 前端不做窄化（未知取值走「不显示」而不是「显示为默认态」）。
 */
export interface NodeFact {
  seq: number;
  node: string;
  iteration: number;
  state: string;
  /** 透传自事件：`true` = 状态迁移，`false` = 仅过程事实（I55 §4.6 第 5 条）。 */
  transition: boolean;
  atMs: number;
  detail: AgentNodeDetail | null;
  source: FactSource;
  /** 合成事实标记：`timeline` 有界丢弃后留下的「已省略 N 条」（I55 §6.7 第 5 条）。 */
  synthetic?: boolean;
  /** 合成事实省略的条数；仅 `synthetic` 为真时有意义。 */
  omitted?: number;
}

/** 一份 turn 观测实例（I55 §6.1）。 */
export interface TurnObservation {
  conversationId: string;
  turnId: string;
  turnIndex: number | null;
  /** 与 `chat-delta` 共用的单调序列（I55 §4.2）。 */
  lastSeq: number;
  /** 最近一条正文增量所属迭代：只服务「模型正在出字」的时间轴，不参与判定。 */
  lastDeltaIteration: number | null;
  startedAtMs: number;
  status: TurnObservationStatus;
  /** key = `${node}:${iteration}`。 */
  cells: Map<string, NodeCell>;
  timeline: NodeFact[];
  /** LRU 淘汰后的摘要实例：只剩 `status`，`cells` / `timeline` 已清空（I55 §6.3 第 3 条）。 */
  evicted?: boolean;
}

/** 格子键：`(node, iteration)` 唯一确定一格。 */
export function cellKey(node: string, iteration: number): string {
  return `${node}:${iteration}`;
}

/** 观测实例键：`${conversationId}:${turnId}`（I55 §6.1）。 */
export function observationKey(conversationId: string, turnId: string): string {
  return `${conversationId}:${turnId}`;
}
