import type { BudgetRemainingDto } from "../../../../types/protocol.contract";
import { currentIterationOf } from "../reducer";
import { isAgentNodeId, templateOrderOf } from "../template";
import {
  cellKey,
  type AgentNodeId,
  type FactSource,
  type NodeFact,
  type TurnObservation,
  type TurnObservationStatus,
} from "../types";

/** 一个格子的耗时（毫秒）。 */
export interface NodeDurationView {
  node: AgentNodeId;
  iteration: number;
  durationMs: number;
  source: FactSource;
}

export interface TurnSummaryView {
  conversationId: string;
  turnId: string;
  status: TurnObservationStatus;
  /** 迭代数 = 最大 `iteration`（`derived`，I55 §5.4 第 3 条）。 */
  iteration: number;
  startedAtMs: number;
  /** 耗时分解：只含已闭合的格子，按 `(iteration, 模板顺序)` 排序。 */
  durations: NodeDurationView[];
  /** 已测得的耗时总和（毫秒）。 */
  measuredMs: number;
  /** 因 `source === "assumed"` 被排除在耗时分解之外的格子数（I55 §6.6 第 4 条）。 */
  assumedCells: number;
  /** ③`brake` 最近一次给出的预算余量；未配置预算的字段为 `null`（I55 §4.4）。 */
  budget: BudgetRemainingDto | null;
}

/**
 * 汇总投影（纯函数，I55 §6.2）。
 *
 * 耗时分解**必须跳过 `source === "assumed"` 的格子**：恢复态只有「看起来在跑」，
 * 起点是补拉时猜的，计入会污染总耗时（§5.3 前提 ③ / §6.6 第 4 条）。
 * `running ⇄ waiting` 互转属于同一段执行（§5.3 前提 ②），不拆成两段。
 * 仍处中间态的格子不给耗时（进行中不等于已耗时），只计已闭合的段。
 */
export function projectTurnSummary(obs: TurnObservation): TurnSummaryView {
  const iteration = currentIterationOf(obs);
  const transitionFacts = obs.timeline.filter(
    (fact) => fact.transition && !fact.synthetic,
  );
  const factsByCell = new Map<string, NodeFact[]>();
  for (const fact of transitionFacts) {
    if (!isAgentNodeId(fact.node)) continue;
    const key = cellKey(fact.node, fact.iteration);
    const list = factsByCell.get(key);
    if (list) list.push(fact);
    else factsByCell.set(key, [fact]);
  }

  let assumedCells = 0;
  for (const cell of obs.cells.values()) {
    if (cell.source === "assumed") assumedCells += 1;
  }

  const durations: NodeDurationView[] = [];
  let measuredMs = 0;
  for (const [key, facts] of factsByCell) {
    const cell = obs.cells.get(key);
    if (!cell || cell.source === "assumed") continue;
    const durationMs = closedDurationOf(facts);
    if (durationMs === null || durationMs <= 0) continue;
    durations.push({
      node: cell.node,
      iteration: cell.iteration,
      durationMs,
      source: cell.source,
    });
    measuredMs += durationMs;
  }

  durations.sort(
    (a, b) =>
      a.iteration - b.iteration ||
      (templateOrderOf(a.node) ?? 0) - (templateOrderOf(b.node) ?? 0),
  );

  return {
    conversationId: obs.conversationId,
    turnId: obs.turnId,
    status: obs.status,
    iteration,
    startedAtMs: obs.startedAtMs,
    durations,
    measuredMs,
    assumedCells,
    budget: budgetOf(obs),
  };
}

/**
 * 一个格子的已闭合耗时：`running` / `waiting` 起算，离开该段（终态或其它状态）时结算。
 * 末尾仍开放（格子仍在进行中）返回 `null`——进行中不产生耗时。
 */
function closedDurationOf(facts: NodeFact[]): number | null {
  let activeStart: number | null = null;
  let total = 0;
  for (const fact of facts) {
    const active = fact.state === "running" || fact.state === "waiting";
    if (active) {
      if (activeStart === null) activeStart = fact.atMs;
      continue;
    }
    if (activeStart !== null) {
      total += fact.atMs - activeStart;
      activeStart = null;
    }
  }
  if (activeStart !== null) return null;
  return total;
}

/** ③`brake` 最近一次（最大 `iteration`）给出的预算余量。 */
function budgetOf(obs: TurnObservation): BudgetRemainingDto | null {
  let best: { iteration: number; budget: BudgetRemainingDto } | null = null;
  for (const cell of obs.cells.values()) {
    if (cell.node !== "brake") continue;
    if (cell.detail?.kind !== "brake") continue;
    if (best === null || cell.iteration >= best.iteration) {
      best = { iteration: cell.iteration, budget: cell.detail.budget };
    }
  }
  return best?.budget ?? null;
}
