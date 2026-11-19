import { createLogger } from "@/services/system/logger";
import type {
  AgentNodeEventDto,
  AgentToolCallDto,
  PersistDetailDto,
  SessionStateDto,
} from "../../../types/protocol.contract";
import {
  cellKey,
  FACT_SOURCE_RANK,
  type AgentNodeDetail,
  type AgentNodeId,
  type NodeCell,
  type NodeFact,
  type NodeState,
  type TurnObservation,
  type TurnObservationStatus,
} from "./types";
import {
  isAgentNodeId,
  isIterationNode,
  isNodeState,
  isTerminalNodeState,
  nodesAfterInIteration,
  TOOL_BRANCH_NODE_IDS,
} from "./template";

const log = createLogger("observation");

/** 单实例时间线上限（I55 §6.7 第 5 条）。 */
export const TIMELINE_LIMIT = 2000;

/** 新建一份观测实例（首条事件到达时）。 */
export function createObservation(
  conversationId: string,
  turnId: string,
  startedAtMs: number,
): TurnObservation {
  return {
    conversationId,
    turnId,
    turnIndex: null,
    lastSeq: 0,
    lastDeltaIteration: null,
    startedAtMs,
    status: "running",
    cells: new Map(),
    timeline: [],
  };
}

/**
 * 折叠一条 `agent-node-event`（I55 §6.7 第 1 条：同步、逐条、不丢弃）。
 *
 * 三条硬规则：
 * 1. `transition === false` 只写时间线，不改格子、不重置 `sinceMs`（G12 读侧）；
 * 2. 终态不可覆盖（O2 / G4），`persist-assistant` 的重复 `finished` 只累加落库行数；
 * 3. 没有真实变化时返回**原引用**（O11 / G14）。
 */
export function applyNodeEvent(
  obs: TurnObservation,
  event: AgentNodeEventDto,
): TurnObservation {
  const node: string = event.node;
  const state: string = event.state;
  const fact: NodeFact = {
    seq: event.seq,
    node,
    iteration: event.iteration,
    state,
    transition: event.transition,
    atMs: event.atMs,
    detail: event.detail,
    source: "observed",
  };

  const known = isAgentNodeId(node) && isNodeState(state);

  if (!event.transition || !known) {
    return appendFact(obs, fact);
  }

  const nodeId: AgentNodeId = node;
  const key = cellKey(nodeId, event.iteration);
  const prevCell = obs.cells.get(key);

  if (prevCell && isTerminalNodeState(prevCell.state)) {
    const merged = mergePersistFinish(nodeId, state, event.detail, prevCell);
    if (!merged) {
      log.debug("终态保护：丢弃覆盖终态的节点事件", {
        node: nodeId,
        iteration: event.iteration,
        state,
      });
      return obs;
    }
    return appendFact(
      { ...obs, cells: withCell(obs.cells, key, merged) },
      fact,
    );
  }

  let cells = obs.cells;
  if (state === "running" && isIterationNode(nodeId)) {
    cells = derivePendingCells(cells, nodeId, event.iteration);
  }
  if (nodeId === "branch" && state === "finished") {
    cells = deriveSkippedCells(cells, event.iteration, event.detail);
  }

  const nextCell: NodeCell = {
    node: nodeId,
    iteration: event.iteration,
    state,
    sinceMs: sinceMsOf(prevCell, event.atMs),
    detail: event.detail,
    source: "observed",
  };
  cells = withCell(cells, key, nextCell);

  return appendFact({ ...obs, cells }, fact);
}

/**
 * 折叠一条 `chat-delta`（I55 §4.7）：只记 `lastSeq` 与「模型正在出字」的迭代号，
 * **不触碰任何格子状态**——正文增量归对话投影，观测面只用它做时间轴对齐。
 */
export function applyDeltaSeq(
  obs: TurnObservation,
  seq: number,
  iteration: number,
): TurnObservation {
  if (seq <= obs.lastSeq && iteration === obs.lastDeltaIteration) return obs;
  return {
    ...obs,
    lastSeq: Math.max(obs.lastSeq, seq),
    lastDeltaIteration: iteration,
  };
}

/**
 * 折叠一条 `agent-tool-call`（I55 §5.2 第 1–2 条）。
 *
 * 只处理两个「阻塞在外部」的相位：审批 / 作答等待把 `pipeline` 置 `waiting`，
 * 审批关闭后回到 `running`。这类 `waiting` 有真实载体，属 `observed`（不是推导）。
 * **禁止**为 `waiting` 新增事件（O5）。
 */
export function applyToolCall(
  obs: TurnObservation,
  call: AgentToolCallDto,
): TurnObservation {
  const phase = call.payload.phase;
  const key = cellKey("pipeline", call.iteration);
  const prevCell = obs.cells.get(key);

  if (phase === "awaitingApproval" || phase === "awaitingAnswer") {
    if (prevCell && isTerminalNodeState(prevCell.state)) return obs;
    if (prevCell?.state === "waiting") return obs;
    return {
      ...obs,
      cells: withCell(obs.cells, key, {
        node: "pipeline",
        iteration: call.iteration,
        state: "waiting",
        sinceMs: sinceMsOf(prevCell, obs.startedAtMs),
        detail: prevCell?.detail ?? null,
        source: "observed",
      }),
    };
  }

  if (phase === "approvalClosed") {
    if (!prevCell || prevCell.state !== "waiting") return obs;
    return {
      ...obs,
      cells: withCell(obs.cells, key, {
        ...prevCell,
        state: "running",
        source: "observed",
      }),
    };
  }

  return obs;
}

/**
 * 折叠一条 `session-state-changed`（V2-a 实例收尾规则）。
 *
 * 只动实例 `status`：仍处 `running` / `waiting` 的格子**保持原状态值**，
 * 由图渲染器显示「未完成」徽标，**不臆造** `skipped` / `cancelled`（O2 / O5）。
 */
export function applySessionState(
  obs: TurnObservation,
  state: SessionStateDto,
): TurnObservation {
  const status = statusOfSessionState(state);
  if (status === obs.status) return obs;
  return { ...obs, status };
}

/**
 * 写入一个补拉得到的格子（I55 §6.4）：`source = assumed`，且**不覆盖**
 * 已有真实事件（`observed`）或已有终态。证据等级只升不降（I55 §6.6 第 2 条）。
 */
export function applyAssumedCell(
  obs: TurnObservation,
  node: AgentNodeId,
  iteration: number,
  state: NodeState,
  atMs: number,
): TurnObservation {
  const key = cellKey(node, iteration);
  const prevCell = obs.cells.get(key);
  if (prevCell && isTerminalNodeState(prevCell.state)) return obs;
  if (prevCell && FACT_SOURCE_RANK[prevCell.source] > FACT_SOURCE_RANK.assumed)
    return obs;
  if (prevCell && prevCell.state === state && prevCell.source === "assumed")
    return obs;
  const nextCell: NodeCell = {
    node,
    iteration,
    state,
    sinceMs: prevCell?.sinceMs ?? atMs,
    detail: prevCell?.detail ?? null,
    source: "assumed",
  };
  return { ...obs, cells: withCell(obs.cells, key, nextCell) };
}

/** 记录本轮 turn 序号（来自 `chat-stream-start`，I55 §4.7）。 */
export function applyTurnIndex(
  obs: TurnObservation,
  turnIndex: number | null,
): TurnObservation {
  if (obs.turnIndex === turnIndex) return obs;
  return { ...obs, turnIndex };
}

/** 实例当前迭代号：迭代内节点已出现的最大 `iteration`（≥ 1）。 */
export function currentIterationOf(obs: TurnObservation): number {
  let max = 0;
  for (const cell of obs.cells.values()) {
    if (cell.iteration > max) max = cell.iteration;
  }
  return Math.max(max, 1);
}

function withCell(
  cells: Map<string, NodeCell>,
  key: string,
  cell: NodeCell,
): Map<string, NodeCell> {
  const next = new Map(cells);
  next.set(key, cell);
  return next;
}

/**
 * `sinceMs` 的三个可信前提之一（I55 §5.3）：`running ⇄ waiting` 互转**不重置**基准，
 * 否则审批等待会把「已持续时长」清零；进入终态同样保留基准，时长度量的是整段执行。
 *
 * 只对 `observed` 成立：`assumed` 格子的起点是补拉时猜的，收到真实事件时必须改用
 * 真实 `atMs`（I55 §5.3 前提 ③ / §6.6 第 4 条）。
 */
function sinceMsOf(prevCell: NodeCell | undefined, atMs: number): number {
  if (
    prevCell &&
    prevCell.source === "observed" &&
    (prevCell.state === "running" || prevCell.state === "waiting")
  )
    return prevCell.sinceMs;
  return atMs;
}

/**
 * `pending` 推导（I55 §4.6 第 1 条）：收到某节点第一条 `running` 时，
 * 把同迭代中排在它后面的节点补成 `pending`（`source = derived`）。后端不发 `pending`。
 */
function derivePendingCells(
  cells: Map<string, NodeCell>,
  node: AgentNodeId,
  iteration: number,
): Map<string, NodeCell> {
  const targets = nodesAfterInIteration(node);
  if (targets.length === 0) return cells;
  let next = cells;
  for (const target of targets) {
    const key = cellKey(target, iteration);
    if (next.has(key)) continue;
    if (next === cells) next = new Map(cells);
    next.set(key, {
      node: target,
      iteration,
      state: "pending",
      sinceMs: 0,
      detail: null,
      source: "derived",
    });
  }
  return next;
}

/**
 * `skipped` 批量标记（I55 §4.6 第 2 条）：⑦ `branch` 的 `finished` 携带
 * `BranchDetail`，无工具调用时把 ⑧–⑫ 一次性标为 `skipped`（`source = derived`）。
 */
function deriveSkippedCells(
  cells: Map<string, NodeCell>,
  iteration: number,
  detail: AgentNodeDetail | null,
): Map<string, NodeCell> {
  if (detail?.kind !== "branch" || detail.hasToolCalls) return cells;
  let next = cells;
  for (const target of TOOL_BRANCH_NODE_IDS) {
    const key = cellKey(target, iteration);
    const prev = next.get(key);
    if (prev && prev.source === "observed") continue;
    if (prev && isTerminalNodeState(prev.state)) continue;
    if (next === cells) next = new Map(cells);
    next.set(key, {
      node: target,
      iteration,
      state: "skipped",
      sinceMs: prev?.sinceMs ?? 0,
      detail: prev?.detail ?? null,
      source: "derived",
    });
  }
  return next;
}

/**
 * `persist-assistant` 的 O2 例外（I55 §4.6 第 4 条）：行已 `finalize` 后只能新建行，
 * 同一迭代可多次落库。重复 `finished` 只累加 `rows` / `messageIds`，状态值不动。
 * 落库行数没变化时返回 `null`（调用方据此丢弃整条事件，O11）。
 */
function mergePersistFinish(
  node: AgentNodeId,
  state: string,
  detail: AgentNodeDetail | null,
  prevCell: NodeCell,
): NodeCell | null {
  if (node !== "persist-assistant" || state !== "finished") return null;
  if (detail?.kind !== "persist") return null;
  const incoming: PersistDetailDto = detail;
  if (incoming.rows === 0) return null;
  const prevDetail =
    prevCell.detail?.kind === "persist" ? prevCell.detail : null;
  const messageIds = [
    ...(prevDetail?.messageIds ?? []),
    ...incoming.messageIds,
  ];
  return {
    ...prevCell,
    detail: {
      kind: "persist",
      rows: (prevDetail?.rows ?? 0) + incoming.rows,
      messageIds,
    },
  };
}

function statusOfSessionState(state: SessionStateDto): TurnObservationStatus {
  if (state === "idle") return "completed";
  if (typeof state === "object" && "error" in state) return "failed";
  return "running";
}

function appendFact(obs: TurnObservation, fact: NodeFact): TurnObservation {
  return {
    ...obs,
    lastSeq: Math.max(obs.lastSeq, fact.seq),
    timeline: trimTimeline([...obs.timeline, fact]),
  };
}

/**
 * 时间线有界（I55 §6.7 第 5 条）：先丢最早的过程事实（`transition: false`），
 * 再丢最早的非首条 `running`；**状态迁移终态永不丢弃**。
 * 丢弃必须留下一条「已省略 N 条」的合成事实（`source = derived`），禁止静默截断。
 */
function trimTimeline(timeline: NodeFact[]): NodeFact[] {
  const facts = timeline.filter((fact) => !fact.synthetic);
  const marker = timeline.find((fact) => fact.synthetic);
  let overflow = facts.length - TIMELINE_LIMIT;
  if (overflow <= 0) return timeline;

  const dropped: NodeFact[] = [];
  const dropWhile = (predicate: (fact: NodeFact) => boolean): void => {
    for (let index = 0; index < facts.length && overflow > 0;) {
      if (predicate(facts[index])) {
        dropped.push(facts[index]);
        facts.splice(index, 1);
        overflow -= 1;
      } else {
        index += 1;
      }
    }
  };

  dropWhile((fact) => !fact.transition);

  const seenRunning = new Set<string>();
  dropWhile((fact) => {
    if (fact.state !== "running" || !fact.transition) return false;
    const key = cellKey(fact.node, fact.iteration);
    if (!seenRunning.has(key)) {
      seenRunning.add(key);
      return false;
    }
    return true;
  });

  const omitted = (marker?.omitted ?? 0) + dropped.length;
  const firstDropped = dropped[0];
  const nextMarker: NodeFact = {
    seq: marker?.seq ?? firstDropped.seq,
    node: "",
    iteration: 0,
    state: "",
    transition: false,
    atMs: marker?.atMs ?? firstDropped.atMs,
    detail: null,
    source: "derived",
    synthetic: true,
    omitted,
  };
  return [nextMarker, ...facts];
}
